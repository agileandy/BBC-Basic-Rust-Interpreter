//! Program run loop for BBC BASIC
//!
//! Runs a stored program from its first line and handles the control flow
//! between lines (GOTO, GOSUB, loops, procedures and ON ERROR).

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;

use crate::error::BBCBasicError;
use crate::executor::{Executor, FunctionBody, ResultEvaluator};
use crate::parser::{parse_statement, Expression, Statement};
use crate::program::ProgramStore;

/// What the run loop does after a statement.
enum Flow {
    /// Continue with the next line
    Next,
    /// The statement set the current line
    Jumped,
    /// Stop the program
    Stop,
    /// A `=expr` line ended a multi-line function
    FnReturn(Expression),
}

/// The parsed lines of the program, shared with the function body runner.
struct ProgramLines {
    statements: HashMap<u16, Statement>,
    line_numbers: BTreeSet<u16>,
}

/// Moves the line position of a run.
trait LineCursor {
    /// Go to a line. Returns false if the line does not exist.
    fn goto_line(&mut self, line_number: u16) -> bool;
    /// Go to the next line and return it (None at the end of the program).
    fn next_line(&mut self) -> Option<u16>;
}

impl LineCursor for ProgramStore {
    fn goto_line(&mut self, line_number: u16) -> bool {
        ProgramStore::goto_line(self, line_number)
    }

    fn next_line(&mut self) -> Option<u16> {
        ProgramStore::next_line(self)
    }
}

/// The line position of a function body run. It is separate from the
/// position of the caller, so the caller continues from its own line.
struct BodyCursor<'a> {
    line_numbers: &'a BTreeSet<u16>,
    current: Option<u16>,
}

impl LineCursor for BodyCursor<'_> {
    fn goto_line(&mut self, line_number: u16) -> bool {
        if self.line_numbers.contains(&line_number) {
            self.current = Some(line_number);
            true
        } else {
            false
        }
    }

    fn next_line(&mut self) -> Option<u16> {
        self.current = self
            .current
            .and_then(|current| self.line_numbers.range((current + 1)..).next().copied());
        self.current
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

    // First pass: parse each line, collect DATA statements and procedure definitions
    let mut statements: HashMap<u16, Statement> = HashMap::new();
    for (line_number, line) in program.list() {
        let statement = parse_statement(line)
            .map_err(|e| format!("Parse error at line {}: {:?}", line_number, e))?;

        if matches!(statement, Statement::Data { .. }) {
            executor
                .collect_data(&statement)
                .map_err(|e| format!("Error collecting DATA at line {}: {:?}", line_number, e))?;
        }

        if let Statement::DefProc { name, params } = &statement {
            executor.define_procedure(name.clone(), line_number, params.clone());
        }

        // Define each function before the run, so that a DEF FN after END works
        if let Statement::DefFn {
            name,
            params,
            expression,
        } = &statement
        {
            let body = match expression {
                Some(expression) => FunctionBody::Expression(expression.clone()),
                None => FunctionBody::Lines(line_number),
            };
            executor.define_function(name.clone(), params.clone(), body);
        }

        statements.insert(line_number, statement);
    }

    let line_numbers = statements.keys().copied().collect();
    let lines = Rc::new(ProgramLines {
        statements,
        line_numbers,
    });
    let failure = Rc::new(RefCell::new(BodyFailure::default()));

    // A call to a multi-line function runs its body lines with this runner
    let runner_lines = Rc::clone(&lines);
    let runner_failure = Rc::clone(&failure);
    executor.set_function_body_runner(Rc::new(move |executor, def_line, evaluate| {
        run_function_body(executor, &runner_lines, &runner_failure, def_line, evaluate)
    }));

    program.start_execution();

    while let Some(line_number) = program.get_current_line() {
        let statement = lines
            .statements
            .get(&line_number)
            .ok_or_else(|| format!("Line {} not found", line_number))?;

        let error = match step(executor, program, &lines.statements, statement, line_number) {
            Ok(Flow::Next) => {
                program.next_line();
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

                if !program.goto_line(handler_line) {
                    return Err(format!(
                        "Error handler line {} not found (from error at line {})",
                        handler_line, error_line
                    ));
                }
            }
        }
    }

    program.stop_execution();
    Ok(())
}

/// Run the body of a multi-line function, from the line after its DEF line
/// to the `=expr` line. `evaluate` gets the expression of the `=expr` line.
///
/// The body run has its own line position. If a body statement fails, the
/// line and any stop of the program go to `failure` for the main run loop.
fn run_function_body(
    executor: &mut Executor,
    lines: &ProgramLines,
    failure: &RefCell<BodyFailure>,
    def_line: u16,
    evaluate: ResultEvaluator,
) -> Result<(), BBCBasicError> {
    let mut cursor = BodyCursor {
        line_numbers: &lines.line_numbers,
        current: Some(def_line),
    };
    cursor.next_line();

    while let Some(line_number) = cursor.current {
        let statement = lines
            .statements
            .get(&line_number)
            .ok_or(BBCBasicError::BadProgram)?;

        let result = match step(
            executor,
            &mut cursor,
            &lines.statements,
            statement,
            line_number,
        ) {
            Ok(Flow::Next) => {
                cursor.next_line();
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

/// Execute one statement and apply its control flow.
fn step(
    executor: &mut Executor,
    program: &mut impl LineCursor,
    statements: &HashMap<u16, Statement>,
    statement: &Statement,
    line_number: u16,
) -> Result<Flow, StepError> {
    // IF: the run loop executes the selected branch, so that a control flow
    // statement in the branch (GOTO, GOSUB, PROC, END) has an effect.
    if let Statement::If {
        condition,
        then_part,
        else_part,
    } = statement
    {
        // In BBC BASIC, 0 is false and non-zero is true
        let branch = if executor.eval_integer(condition)? != 0 {
            Some(then_part)
        } else {
            else_part.as_ref()
        };
        for branch_statement in branch.into_iter().flatten() {
            let flow = step(executor, program, statements, branch_statement, line_number)?;
            if !matches!(flow, Flow::Next) {
                return Ok(flow);
            }
        }
        return Ok(Flow::Next);
    }

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
            // Push the current line number so RETURN can come back here
            executor.push_gosub_return(line_number);
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
            // Index out of range: continue with the next line
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
                executor.push_gosub_return(line_number);
                if !program.goto_line(target) {
                    return Err(format!("Line {} not found (ON GOSUB)", target).into());
                }
                return Ok(Flow::Jumped);
            }
            // Index out of range: continue with the next line
            Ok(Flow::Next)
        }
        Statement::Return { .. } => {
            let return_line = executor
                .pop_gosub_return()
                .map_err(|_| "RETURN without GOSUB".to_string())?;
            // Jump back to the GOSUB line, then continue with the line after it
            if !program.goto_line(return_line) {
                return Err(format!("Return line {} not found", return_line).into());
            }
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
            let proc_line = proc.line_number;
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

            executor.push_gosub_return(line_number);

            if !program.goto_line(proc_line) {
                return Err(format!("Procedure {} line {} not found", name, proc_line).into());
            }

            // Continue with the line after DEF PROC (skip the definition line)
            Ok(Flow::Next)
        }
        Statement::EndProc => {
            executor
                .exit_local_scope()
                .map_err(|e| format!("Error exiting local scope: {:?}", e))?;

            let return_line = executor
                .pop_gosub_return()
                .map_err(|_| "ENDPROC without PROC call".to_string())?;
            // Jump back to the PROC call line, then continue with the line after it
            if !program.goto_line(return_line) {
                return Err(format!("Return line {} not found", return_line).into());
            }
            Ok(Flow::Next)
        }
        Statement::For { .. } => {
            // Record this line number for NEXT to loop back to
            executor.set_for_loop_line(line_number);
            Ok(Flow::Next)
        }
        Statement::Next { .. } => {
            if let Some(for_line) = executor.should_loop_back() {
                // Loop continues: go to the line after the FOR statement
                if !program.goto_line(for_line) {
                    return Err(format!("FOR loop line {} not found", for_line).into());
                }
            }
            Ok(Flow::Next)
        }
        Statement::Repeat => {
            // Push this line number for UNTIL to loop back to
            executor.push_repeat(line_number);
            Ok(Flow::Next)
        }
        Statement::Until { condition } => {
            let repeat_line = executor
                .check_until(condition)
                .map_err(|e| format!("Error evaluating UNTIL condition: {:?}", e))?;
            if let Some(repeat_line) = repeat_line {
                // Condition false: loop back to the line after REPEAT
                if !program.goto_line(repeat_line) {
                    return Err(format!("REPEAT line {} not found", repeat_line).into());
                }
            }
            Ok(Flow::Next)
        }
        Statement::While { condition } => {
            let entered = executor
                .push_while(line_number, condition)
                .map_err(|e| format!("Error evaluating WHILE condition: {:?}", e))?;
            if entered.is_none() {
                // Condition false: scan forward to the matching ENDWHILE
                let mut depth = 1;
                while depth > 0 {
                    let Some(current_line) = program.next_line() else {
                        return Err("WHILE without matching ENDWHILE".to_string().into());
                    };
                    match statements.get(&current_line) {
                        Some(Statement::While { .. }) => depth += 1,
                        Some(Statement::EndWhile) => depth -= 1,
                        _ => {}
                    }
                }
            }
            // Enter the loop body, or continue after ENDWHILE
            Ok(Flow::Next)
        }
        Statement::EndWhile => {
            // The condition is in the matching WHILE statement
            let while_line = executor
                .check_endwhile_get_while_line()
                .ok_or_else(|| "ENDWHILE without matching WHILE".to_string())?;
            let condition = match statements.get(&while_line) {
                Some(Statement::While { condition }) => condition,
                Some(_) => {
                    return Err(
                        format!("Could not parse WHILE statement at line {}", while_line).into(),
                    );
                }
                None => return Err(format!("WHILE line {} not found", while_line).into()),
            };
            let loop_line = executor
                .check_endwhile(condition)
                .map_err(|e| format!("Error evaluating WHILE condition at ENDWHILE: {:?}", e))?;
            if let Some(loop_line) = loop_line {
                // Condition still true: loop back to the line after WHILE
                if !program.goto_line(loop_line) {
                    return Err(format!("WHILE line {} not found", loop_line).into());
                }
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
        BBCBasicError::NoSuchVariable(_) => 26,
        BBCBasicError::ArrayNotDimensioned(_) => 14,
        BBCBasicError::SyntaxError { .. } => 220,
        BBCBasicError::BadProgram => 254,
        BBCBasicError::IllegalFunction => 31,
        _ => 255,
    }
}
