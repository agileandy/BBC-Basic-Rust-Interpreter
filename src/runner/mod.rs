//! Program run loop for BBC BASIC
//!
//! Runs a stored program from its first line and handles the control flow
//! between statements and lines (GOTO, GOSUB, IF, loops, procedures and
//! ON ERROR).

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::ops::Bound;
use std::rc::Rc;

use crate::error::BBCBasicError;
use crate::executor::{Executor, FunctionBody, ResultEvaluator};
use crate::parser::{parse_line, Expression, Statement};
use crate::program::{Position, ProgramStore};

/// What the run loop does after a statement.
enum Flow {
    /// Continue with the next statement
    Next,
    /// The statement set the current position
    Jumped,
    /// Stop the program
    Stop,
    /// A `=expr` statement ended a multi-line function
    FnReturn(Expression),
}

/// One item of the run list of a line.
///
/// An IF becomes a list of items, so that each statement in a THEN or ELSE
/// branch has a position. A GOSUB, PROC or loop in a branch can then come
/// back to the statement after it.
enum Item {
    /// A statement to execute
    Statement(Statement),
    /// The condition of an IF. If it is false, the run goes to the item at
    /// `otherwise` on the same line (the ELSE branch or the end of the line).
    If {
        condition: Expression,
        otherwise: usize,
    },
    /// The end of a THEN branch: the run goes to the item at this index on
    /// the same line (after the ELSE branch).
    Skip(usize),
}

/// Add the run list items of the statements to `items`.
fn flatten(statements: Vec<Statement>, items: &mut Vec<Item>) {
    for statement in statements {
        let Statement::If {
            condition,
            then_part,
            else_part,
        } = statement
        else {
            items.push(Item::Statement(statement));
            continue;
        };

        let if_index = items.len();
        items.push(Item::If {
            condition,
            otherwise: 0,
        });
        flatten(then_part, items);

        let otherwise = match else_part {
            Some(else_part) => {
                let skip_index = items.len();
                items.push(Item::Skip(0));
                let else_start = items.len();
                flatten(else_part, items);
                items[skip_index] = Item::Skip(items.len());
                else_start
            }
            None => items.len(),
        };
        if let Item::If {
            otherwise: slot, ..
        } = &mut items[if_index]
        {
            *slot = otherwise;
        }
    }
}

/// The parsed lines of the program, shared with the function body runner.
struct ProgramLines {
    items: HashMap<u16, Vec<Item>>,
    line_numbers: BTreeSet<u16>,
}

impl ProgramLines {
    /// The item at a position
    fn item(&self, position: Position) -> Option<&Item> {
        self.items.get(&position.line)?.get(position.index)
    }

    /// The statement at a position (None for an IF item)
    fn statement(&self, position: Position) -> Option<&Statement> {
        match self.item(position)? {
            Item::Statement(statement) => Some(statement),
            _ => None,
        }
    }
}

/// The position of a run. A function body run has its own cursor, so the
/// caller continues from its own position.
struct Cursor<'a> {
    lines: &'a ProgramLines,
    current: Option<Position>,
}

impl Cursor<'_> {
    /// Go to the first statement of a line. Returns false if the line does
    /// not exist.
    fn goto_line(&mut self, line_number: u16) -> bool {
        if self.lines.line_numbers.contains(&line_number) {
            self.current = Some(Position::line_start(line_number));
            true
        } else {
            false
        }
    }

    /// Go to a position. A position after the last item of its line is the
    /// first statement of the next line.
    fn goto(&mut self, position: Position) {
        let line_length = self.lines.items.get(&position.line).map_or(0, Vec::len);
        self.current = if position.index < line_length {
            Some(position)
        } else {
            self.lines
                .line_numbers
                .range((Bound::Excluded(position.line), Bound::Unbounded))
                .next()
                .map(|line| Position::line_start(*line))
        };
    }

    /// Go to the next statement and return it (None at the end of the program).
    fn next(&mut self) -> Option<Position> {
        if let Some(current) = self.current {
            self.goto(Position {
                line: current.line,
                index: current.index + 1,
            });
        }
        self.current
    }

    /// Go to the last item of the current line, so that `next` goes to the
    /// first statement of the next line.
    fn skip_rest_of_line(&mut self) {
        if let Some(current) = self.current.as_mut() {
            let line_length = self.lines.items.get(&current.line).map_or(0, Vec::len);
            current.index = line_length.saturating_sub(1).max(current.index);
        }
    }
}

/// Why a function body run stopped the program instead of giving a BASIC error.
enum BodyAbort {
    /// An error that stops the program
    Fatal(String),
    /// END or STOP in the function body
    Stop,
}

/// What a failed function body run tells the main run loop.
///
/// The error itself goes back through the expression evaluation of the
/// caller, which does not know the line of the body statement.
#[derive(Default)]
struct BodyFailure {
    /// Line of the deepest body statement that failed (for ERL and the message)
    line: Option<u16>,
    /// Set when the program must stop
    abort: Option<BodyAbort>,
}

/// Why a statement failed.
enum StepError {
    /// A BASIC error. An ON ERROR handler can catch it.
    Runtime(BBCBasicError),
    /// An error that stops the program.
    Fatal(String),
}

impl From<BBCBasicError> for StepError {
    fn from(error: BBCBasicError) -> Self {
        StepError::Runtime(error)
    }
}

impl From<String> for StepError {
    fn from(message: String) -> Self {
        StepError::Fatal(message)
    }
}

/// Run the stored program from its first line.
///
/// Each line is parsed one time before the run starts.
pub fn run_program(executor: &mut Executor, program: &mut ProgramStore) -> Result<(), String> {
    if program.is_empty() {
        return Err("No program to run".to_string());
    }

    // Reset and collect all DATA statements before execution begins.
    // This lets READ access DATA regardless of program flow (GOTO, etc.)
    executor.reset_data();
    executor.clear_procedures();
    executor.clear_functions();
    executor.clear_control_stacks();

    // First pass: parse each line, collect DATA statements and procedure definitions
    let mut all_items: HashMap<u16, Vec<Item>> = HashMap::new();
    for (line_number, line) in program.list() {
        let statements = parse_line(line)
            .map_err(|e| format!("Parse error at line {}: {:?}", line_number, e))?;
        let mut items = Vec::new();
        flatten(statements, &mut items);

        for (index, item) in items.iter().enumerate() {
            let Item::Statement(statement) = item else {
                continue;
            };
            let position = Position {
                line: line_number,
                index,
            };

            if matches!(statement, Statement::Data { .. }) {
                executor.collect_data(statement, line_number).map_err(|e| {
                    format!("Error collecting DATA at line {}: {:?}", line_number, e)
                })?;
            }

            if let Statement::DefProc { name, params } = statement {
                executor.define_procedure(name.clone(), position, params.clone());
            }

            // Define each function before the run, so that a DEF FN after END works
            if let Statement::DefFn {
                name,
                params,
                expression,
            } = statement
            {
                let body = match expression {
                    Some(expression) => FunctionBody::Expression(expression.clone()),
                    None => FunctionBody::Lines(position),
                };
                executor.define_function(name.clone(), params.clone(), body);
            }
        }

        all_items.insert(line_number, items);
    }

    let line_numbers = all_items.keys().copied().collect();
    let lines = Rc::new(ProgramLines {
        items: all_items,
        line_numbers,
    });
    let failure = Rc::new(RefCell::new(BodyFailure::default()));

    // A call to a multi-line function runs its body with this runner
    let runner_lines = Rc::clone(&lines);
    let runner_failure = Rc::clone(&failure);
    executor.set_function_body_runner(Rc::new(move |executor, def_position, evaluate| {
        run_function_body(
            executor,
            &runner_lines,
            &runner_failure,
            def_position,
            evaluate,
        )
    }));

    let mut cursor = Cursor {
        lines: &lines,
        current: None,
    };
    if let Some(first_line) = lines.line_numbers.first() {
        cursor.goto_line(*first_line);
    }

    while let Some(position) = cursor.current {
        let line_number = position.line;
        let item = lines
            .item(position)
            .ok_or_else(|| format!("Line {} not found", line_number))?;

        let error = match step(executor, &mut cursor, item, position) {
            Ok(Flow::Next) => {
                cursor.next();
                continue;
            }
            Ok(Flow::Jumped) => continue,
            Ok(Flow::Stop) => break,
            Ok(Flow::FnReturn(_)) => {
                return Err(format!("= without FN call at line {}", line_number));
            }
            Err(error) => error,
        };

        // An error in a function body: use the line of the body statement
        let body = std::mem::take(&mut *failure.borrow_mut());
        let error_line = body.line.unwrap_or(line_number);

        match (body.abort, error) {
            (Some(BodyAbort::Stop), _) => break,
            (Some(BodyAbort::Fatal(message)), _) | (None, StepError::Fatal(message)) => {
                return Err(message);
            }
            (None, StepError::Runtime(e)) => {
                // Handle the error with the ON ERROR handler if one is set
                let Some(handler_line) = executor.get_error_handler() else {
                    return Err(format!("Runtime error at line {}: {:?}", error_line, e));
                };

                // Set error information (ERL and ERR)
                executor.set_last_error(error_number(&e), error_line, format!("{:?}", e));

                if !cursor.goto_line(handler_line) {
                    return Err(format!(
                        "Error handler line {} not found (from error at line {})",
                        handler_line, error_line
                    ));
                }
            }
        }
    }

    Ok(())
}

/// Run the body of a multi-line function, from the statement after its DEF
/// statement to the `=expr` statement. `evaluate` gets the expression of the
/// `=expr` statement.
///
/// The body run has its own position. If a body statement fails, the line
/// and any stop of the program go to `failure` for the main run loop.
fn run_function_body(
    executor: &mut Executor,
    lines: &ProgramLines,
    failure: &RefCell<BodyFailure>,
    def_position: Position,
    evaluate: ResultEvaluator,
) -> Result<(), BBCBasicError> {
    let def_line = def_position.line;
    let mut cursor = Cursor {
        lines,
        current: Some(def_position),
    };
    cursor.next();

    while let Some(position) = cursor.current {
        let line_number = position.line;
        let item = lines.item(position).ok_or(BBCBasicError::BadProgram)?;

        let result = match step(executor, &mut cursor, item, position) {
            Ok(Flow::Next) => {
                cursor.next();
                continue;
            }
            Ok(Flow::Jumped) => continue,
            Ok(Flow::Stop) => return abort_program(failure, line_number, BodyAbort::Stop),
            Ok(Flow::FnReturn(value)) => evaluate(executor, &value).map_err(StepError::Runtime),
            Err(error) => Err(error),
        };

        return match result {
            Ok(()) => {
                // The caller ignored the errors of earlier body runs (for
                // example a PRINT that tries each type). Forget their line.
                let mut failure = failure.borrow_mut();
                if failure.abort.is_none() {
                    failure.line = None;
                }
                Ok(())
            }
            Err(StepError::Runtime(e)) => {
                // Keep the line of the deepest failed statement
                failure.borrow_mut().line.get_or_insert(line_number);
                Err(e)
            }
            Err(StepError::Fatal(message)) => {
                abort_program(failure, line_number, BodyAbort::Fatal(message))
            }
        };
    }

    // The body ran past the last line of the program
    let message = format!("Function at line {} has no = line", def_line);
    abort_program(failure, def_line, BodyAbort::Fatal(message))
}

/// Record that a function body stops the program. The returned error only
/// unwinds the callers. The main run loop reads `failure` and stops.
fn abort_program(
    failure: &RefCell<BodyFailure>,
    line_number: u16,
    abort: BodyAbort,
) -> Result<(), BBCBasicError> {
    let mut failure = failure.borrow_mut();
    failure.line.get_or_insert(line_number);
    failure.abort.get_or_insert(abort);
    Err(BBCBasicError::BadCall)
}

/// Execute one item at `position` and apply its control flow.
fn step(
    executor: &mut Executor,
    program: &mut Cursor,
    item: &Item,
    position: Position,
) -> Result<Flow, StepError> {
    let line_number = position.line;
    let statement = match item {
        Item::Statement(statement) => statement,
        Item::If {
            condition,
            otherwise,
        } => {
            // In BBC BASIC, 0 is false and non-zero is true
            if executor.eval_integer(condition)? != 0 {
                return Ok(Flow::Next);
            }
            program.goto(Position {
                line: line_number,
                index: *otherwise,
            });
            return Ok(Flow::Jumped);
        }
        Item::Skip(index) => {
            program.goto(Position {
                line: line_number,
                index: *index,
            });
            return Ok(Flow::Jumped);
        }
    };

    executor.execute_statement(statement)?;

    match statement {
        Statement::End | Statement::Stop => Ok(Flow::Stop),
        Statement::Goto {
            line_number: target,
        } => {
            if !program.goto_line(*target) {
                return Err(format!("Line {} not found (GOTO)", target).into());
            }
            Ok(Flow::Jumped)
        }
        Statement::Gosub {
            line_number: target,
        } => {
            // Push the current position so RETURN can come back here
            executor.push_gosub_return(position);
            if !program.goto_line(*target) {
                return Err(format!("Line {} not found (GOSUB)", target).into());
            }
            Ok(Flow::Jumped)
        }
        Statement::OnGoto {
            expression,
            targets,
        } => {
            // BBC BASIC uses 1-based indexing
            let index = executor
                .eval_integer(expression)
                .map_err(|e| format!("Error evaluating ON GOTO expression: {:?}", e))?;

            if index >= 1 && (index as usize) <= targets.len() {
                let target = targets[(index - 1) as usize];
                if !program.goto_line(target) {
                    return Err(format!("Line {} not found (ON GOTO)", target).into());
                }
                return Ok(Flow::Jumped);
            }
            // Index out of range: continue with the next statement
            Ok(Flow::Next)
        }
        Statement::OnGosub {
            expression,
            targets,
        } => {
            // BBC BASIC uses 1-based indexing
            let index = executor
                .eval_integer(expression)
                .map_err(|e| format!("Error evaluating ON GOSUB expression: {:?}", e))?;

            if index >= 1 && (index as usize) <= targets.len() {
                let target = targets[(index - 1) as usize];
                executor.push_gosub_return(position);
                if !program.goto_line(target) {
                    return Err(format!("Line {} not found (ON GOSUB)", target).into());
                }
                return Ok(Flow::Jumped);
            }
            // Index out of range: continue with the next statement
            Ok(Flow::Next)
        }
        Statement::Return { .. } => {
            let return_position = executor
                .pop_gosub_return()
                .map_err(|_| "RETURN without GOSUB".to_string())?;
            // Go back to the GOSUB, then continue with the statement after it
            program.goto(return_position);
            Ok(Flow::Next)
        }
        Statement::ProcCall { name, args } => {
            let proc = executor
                .get_procedure(name)
                .ok_or_else(|| format!("Procedure {} not defined", name))?;

            if args.len() != proc.params.len() {
                return Err(format!(
                    "Procedure {} expects {} parameters, got {}",
                    name,
                    proc.params.len(),
                    args.len()
                )
                .into());
            }

            // Clone procedure data before entering local scope
            let proc_position = proc.position;
            let params_and_args: Vec<_> = proc
                .params
                .iter()
                .zip(args.iter())
                .map(|(p, a)| (p.clone(), a.clone()))
                .collect();

            executor.enter_local_scope();

            // Bind arguments to parameters (as global variables)
            for (param_name, arg_expr) in params_and_args {
                executor
                    .execute_statement(&Statement::Assignment {
                        target: param_name,
                        expression: arg_expr,
                    })
                    .map_err(|e| format!("Error binding parameter: {:?}", e))?;
            }

            executor.push_gosub_return(position);

            // Continue with the statement after DEF PROC
            program.goto(proc_position);
            Ok(Flow::Next)
        }
        Statement::EndProc => {
            executor
                .exit_local_scope()
                .map_err(|e| format!("Error exiting local scope: {:?}", e))?;

            let return_position = executor
                .pop_gosub_return()
                .map_err(|_| "ENDPROC without PROC call".to_string())?;
            // Go back to the PROC call, then continue with the statement after it
            program.goto(return_position);
            Ok(Flow::Next)
        }
        Statement::For { .. } => {
            // Record this position for NEXT to loop back to
            executor.set_for_loop_line(position);
            Ok(Flow::Next)
        }
        Statement::Next { .. } => {
            if let Some(for_position) = executor.should_loop_back() {
                // Loop continues: go to the statement after the FOR statement
                program.goto(for_position);
            }
            Ok(Flow::Next)
        }
        Statement::Repeat => {
            // Push this position for UNTIL to loop back to
            executor.push_repeat(position);
            Ok(Flow::Next)
        }
        Statement::Until { condition } => {
            let repeat_position = executor
                .check_until(condition)
                .map_err(|e| format!("Error evaluating UNTIL condition: {:?}", e))?;
            if let Some(repeat_position) = repeat_position {
                // Condition false: loop back to the statement after REPEAT
                program.goto(repeat_position);
            }
            Ok(Flow::Next)
        }
        Statement::While { condition } => {
            let entered = executor
                .push_while(position, condition)
                .map_err(|e| format!("Error evaluating WHILE condition: {:?}", e))?;
            if entered.is_none() {
                // Condition false: scan forward to the matching ENDWHILE
                let mut depth = 1;
                while depth > 0 {
                    let Some(current) = program.next() else {
                        return Err("WHILE without matching ENDWHILE".to_string().into());
                    };
                    match program.lines.item(current) {
                        // An IF uses the rest of its line. A WHILE or ENDWHILE
                        // in its branches does not count.
                        Some(Item::If { .. }) => program.skip_rest_of_line(),
                        Some(Item::Statement(Statement::While { .. })) => depth += 1,
                        Some(Item::Statement(Statement::EndWhile)) => depth -= 1,
                        _ => {}
                    }
                }
            }
            // Enter the loop body, or continue after ENDWHILE
            Ok(Flow::Next)
        }
        Statement::EndWhile => {
            // The condition is in the matching WHILE statement
            let while_position = executor
                .check_endwhile_get_while_line()
                .ok_or_else(|| "ENDWHILE without matching WHILE".to_string())?;
            let Some(Statement::While { condition }) = program.lines.statement(while_position)
            else {
                return Err(format!(
                    "Could not find WHILE statement at line {}",
                    while_position.line
                )
                .into());
            };
            let loop_position = executor
                .check_endwhile(condition)
                .map_err(|e| format!("Error evaluating WHILE condition at ENDWHILE: {:?}", e))?;
            if let Some(loop_position) = loop_position {
                // Condition still true: loop back to the statement after WHILE
                program.goto(loop_position);
            }
            Ok(Flow::Next)
        }
        Statement::FnReturn { value } => Ok(Flow::FnReturn(value.clone())),
        _ => Ok(Flow::Next),
    }
}

/// Convert an interpreter error to its BBC BASIC error number (ERR).
fn error_number(error: &BBCBasicError) -> i32 {
    match error {
        BBCBasicError::DivisionByZero => 18,
        BBCBasicError::TypeMismatch => 6,
        BBCBasicError::SubscriptOutOfRange => 15,
        BBCBasicError::NoRoom => 11,
        BBCBasicError::StringTooLong => 19,
        BBCBasicError::TooBig => 20,
        BBCBasicError::NoSuchVariable(_) => 26,
        BBCBasicError::ArrayNotDimensioned(_) => 14,
        BBCBasicError::SyntaxError { .. } => 220,
        BBCBasicError::BadProgram => 254,
        BBCBasicError::IllegalFunction => 31,
        _ => 255,
    }
}
