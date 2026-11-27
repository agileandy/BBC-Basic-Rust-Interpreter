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
use crate::program::{Position, ProgramStore, TYPED_LINE};
use crate::tokenizer::TokenizedLine;

/// What the run loop does after a statement.
enum Flow {
    /// Continue with the next statement
    Next,
    /// The statement set the current position
    Jumped,
    /// Stop the program
    Stop,
    /// Stop the program and leave the interpreter
    Quit,
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
    /// `otherwise` on the same line (after the next ELSE on the line, or the
    /// end of the line).
    If {
        condition: Expression,
        otherwise: usize,
    },
    /// An ELSE: the run goes to the item at this index on the same line (the
    /// end of the line).
    Skip(usize),
    /// A stored line that does not parse. The run stops with this parse
    /// error when it gets to the line.
    Invalid(String),
}

/// Make the run list of the statements of one line.
///
/// The items keep the order of the line as typed. In BBC BASIC a false IF
/// continues after the next ELSE on the line, at any depth, and the run
/// skips the rest of the line when it gets to an ELSE.
fn flatten_line(statements: Vec<Statement>) -> Vec<Item> {
    let mut items = Vec::new();
    flatten(statements, &mut items);

    let end = items.len();
    let mut after_next_else = end;
    for index in (0..end).rev() {
        match &mut items[index] {
            Item::Skip(target) => {
                *target = end;
                after_next_else = index + 1;
            }
            Item::If { otherwise, .. } => *otherwise = after_next_else,
            Item::Statement(_) | Item::Invalid(_) => {}
        }
    }
    items
}

/// Add the run list items of the statements to `items`, in the order of the
/// line. `flatten_line` sets the jump indexes.
fn flatten(statements: Vec<Statement>, items: &mut Vec<Item>) {
    for mut statement in statements {
        // The ELSE part of an ON follows it as an ELSE, as for an IF. An
        // index out of range continues after the next ELSE on the line.
        if let Statement::OnGoto { else_part, .. } | Statement::OnGosub { else_part, .. } =
            &mut statement
        {
            let else_part = else_part.take();
            items.push(Item::Statement(statement));
            if let Some(else_part) = else_part {
                items.push(Item::Skip(0));
                flatten(else_part, items);
            }
            continue;
        }

        let Statement::If {
            condition,
            then_part,
            else_part,
        } = statement
        else {
            items.push(Item::Statement(statement));
            continue;
        };

        items.push(Item::If {
            condition,
            otherwise: 0,
        });
        flatten(then_part, items);
        if let Some(else_part) = else_part {
            items.push(Item::Skip(0));
            flatten(else_part, items);
        }
    }
}

/// The parsed lines of the program, shared with the function body runner.
struct ProgramLines {
    items: HashMap<u32, Vec<Item>>,
    /// The stored line numbers. The typed line is not in this set, so a jump
    /// or the end of a stored line never goes to it.
    line_numbers: BTreeSet<u32>,
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
        if self.lines.line_numbers.contains(&u32::from(line_number)) {
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
                .map(|line| Position {
                    line: *line,
                    index: 0,
                })
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

    /// Go to the item after the next ELSE on the line of `position`, at any
    /// depth. This is where an ON with an index out of range continues. With
    /// no ELSE after `position`, the error is "ON range".
    fn goto_after_next_else(&mut self, position: Position) -> Result<(), BBCBasicError> {
        let items = self
            .lines
            .items
            .get(&position.line)
            .map_or(&[][..], Vec::as_slice);
        let else_index = items
            .iter()
            .skip(position.index + 1)
            .position(|item| matches!(item, Item::Skip(_)))
            .ok_or(BBCBasicError::OnRange)?;
        self.goto(Position {
            line: position.line,
            index: position.index + 1 + else_index + 1,
        });
        Ok(())
    }

    /// Go to the last item of the current line, so that `next` goes to the
    /// first statement of the next line.
    fn skip_rest_of_line(&mut self) {
        if let Some(current) = self.current.as_mut() {
            let line_length = self.lines.items.get(&current.line).map_or(0, Vec::len);
            current.index = line_length.saturating_sub(1).max(current.index);
        }
    }

    /// Go to the statement after the ELSE or ENDIF line that matches the
    /// block IF or the ELSE line on `line`. With `stop_at_else` false, only
    /// the matching ENDIF line stops the scan. Returns None if no line
    /// matches before the end of the program.
    ///
    /// A line that ends with a block IF opens a nested block. This includes
    /// the ELSE line on `line` (an `ELSE IF ... THEN` line). Only an ELSE or
    /// ENDIF at the start of a line counts.
    fn skip_block(&mut self, line: u32, stop_at_else: bool) -> Option<()> {
        let else_opens_block = !stop_at_else
            && matches!(
                self.lines.items.get(&line).and_then(|items| items.last()),
                Some(Item::Statement(Statement::IfBlock { .. }))
            );
        let mut depth = if else_opens_block { 2 } else { 1 };
        let after = (Bound::Excluded(line), Bound::Unbounded);
        for &line in self.lines.line_numbers.range(after) {
            let items = &self.lines.items[&line];
            let stop = match items.first() {
                Some(Item::Statement(Statement::EndIf)) => {
                    depth -= 1;
                    depth == 0
                }
                Some(Item::Statement(Statement::BlockElse)) => stop_at_else && depth == 1,
                _ => false,
            };
            if stop {
                self.goto(Position { line, index: 1 });
                return Some(());
            }
            if let Some(Item::Statement(Statement::IfBlock { .. })) = items.last() {
                depth += 1;
            }
        }
        None
    }
}

/// Why a function body run stopped the program instead of giving a BASIC error.
enum BodyAbort {
    /// An error that stops the program
    Fatal(String),
    /// END or STOP in the function body
    Stop,
    /// QUIT in the function body
    Quit,
}

/// What a failed function body run tells the main run loop.
///
/// The error itself goes back through the expression evaluation of the
/// caller, which does not know the line of the body statement.
#[derive(Default)]
struct BodyFailure {
    /// Line of the deepest body statement that failed (for ERL and the message)
    line: Option<u32>,
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

/// How a program run ended without an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOutcome {
    /// The program ended (END, STOP or past its last line)
    Ended,
    /// The program ran QUIT. The caller leaves the interpreter.
    Quit,
}

/// Run the stored program from its first line.
///
/// Each line is parsed one time before the run starts.
pub fn run_program(executor: &mut Executor, program: &mut ProgramStore) -> Result<(), String> {
    run_program_with_outcome(executor, program).map(|_| ())
}

/// Run the stored program from its first line and tell how it ended.
pub fn run_program_with_outcome(
    executor: &mut Executor,
    program: &mut ProgramStore,
) -> Result<RunOutcome, String> {
    if program.is_empty() {
        return Err("No program to run".to_string());
    }

    // Reset and collect all DATA statements before execution begins.
    // This lets READ access DATA regardless of program flow (GOTO, etc.)
    executor.reset_data();
    executor.clear_procedures();
    executor.clear_functions();
    // RUN starts with no variables, except A% to Z%
    executor.clear_variables();
    // RUN starts with no ON ERROR handler. CLEAR keeps the handler.
    executor.clear_error_handler();
    executor.set_program_size(program.size_in_bytes());
    executor.reset_column();

    // First pass: parse each line, collect DATA statements and procedure definitions
    let mut all_items: HashMap<u32, Vec<Item>> = HashMap::new();
    for (line_number, line) in program.list() {
        let statements = parse_line(line).map_err(|e| error_text(&e, line_number.into()))?;
        let items = flatten_line(statements);

        for item in &items {
            if let Item::Statement(statement @ Statement::Data { .. }) = item {
                executor
                    .collect_data(statement, line_number)
                    .map_err(|e| error_text(&e, line_number.into()))?;
            }
        }
        define_routines(executor, line_number, &items);

        all_items.insert(line_number.into(), items);
    }

    let line_numbers = all_items.keys().copied().collect();
    let lines = Rc::new(ProgramLines {
        items: all_items,
        line_numbers,
    });
    check_if_blocks(&lines)?;
    let failure = install_function_body_runner(executor, &lines);

    let start = lines.line_numbers.first().map(|line| Position {
        line: *line,
        index: 0,
    });
    let local_depth = executor.local_depth();
    let outcome = run_lines(executor, &lines, &failure, start);
    // A run that stops inside a PROC or FN restores its LOCAL variables
    executor.exit_local_scopes_to(local_depth);
    outcome
}

/// Run a line typed with no line number, with the same control flow as a
/// program line.
///
/// A jump or a call goes to the stored program. GOSUB, PROC and FN come back
/// to the typed line. GOTO continues in the stored program. The variables
/// and the functions defined in immediate mode stay. The control stacks and
/// the LOCAL scopes are as before the line, also after an error. An error in
/// a stored line goes to the ON ERROR handler. An error in the typed line
/// does not.
pub fn run_typed_line(
    executor: &mut Executor,
    program: &ProgramStore,
    line: &TokenizedLine,
) -> Result<RunOutcome, String> {
    let statements = parse_line(line).map_err(|e| error_text(&e, TYPED_LINE))?;
    let typed_items = flatten_line(statements);
    for item in &typed_items {
        match item {
            Item::Statement(Statement::DefProc { .. }) => {
                return Err("DEF PROC needs a line number".to_string());
            }
            Item::Statement(Statement::DefFn {
                expression: None, ..
            }) => return Err("DEF FN needs a line number".to_string()),
            // The block is on the next lines, and a typed line has none
            Item::Statement(Statement::IfBlock { .. }) => {
                return Err("Block IF needs a line number".to_string());
            }
            _ => {}
        }
    }

    // Define the procedures and functions of the stored program again, so
    // that a changed or deleted DEF does not keep an old position
    executor.clear_procedures();
    executor.clear_multi_line_functions();
    executor.set_program_size(program.size_in_bytes());
    executor.reset_column();

    let mut all_items: HashMap<u32, Vec<Item>> = HashMap::new();
    let mut data_values = Vec::new();
    let mut data_line_numbers = Vec::new();
    for (line_number, stored) in program.list() {
        let items = match parse_line(stored) {
            Ok(statements) => flatten_line(statements),
            // The run gives the parse error only if it gets to this line
            Err(e) => vec![Item::Invalid(error_text(&e, line_number.into()))],
        };
        for item in &items {
            if let Item::Statement(Statement::Data { values }) = item {
                data_values.extend(values.iter().cloned());
                data_line_numbers.extend(values.iter().map(|_| Some(line_number)));
            }
        }
        define_routines(executor, line_number, &items);
        all_items.insert(line_number.into(), items);
    }
    executor.use_data_pool(data_values, data_line_numbers);

    let line_numbers = all_items.keys().copied().collect();
    all_items.insert(TYPED_LINE, typed_items);
    let lines = Rc::new(ProgramLines {
        items: all_items,
        line_numbers,
    });
    // A stored line that does not parse is an Invalid item, and the check
    // does not count it
    check_if_blocks(&lines)?;
    let failure = install_function_body_runner(executor, &lines);

    executor.clear_control_stacks();
    let local_depth = executor.local_depth();
    let start = Some(Position {
        line: TYPED_LINE,
        index: 0,
    });
    let outcome = run_lines(executor, &lines, &failure, start);
    executor.exit_local_scopes_to(local_depth);
    executor.clear_control_stacks();
    outcome
}

/// Make sure that each block IF has a matching ENDIF line, with the count
/// rules of `Cursor::skip_block`. Without this check, a true block with no
/// ENDIF runs to the end of the program with no error.
///
/// An ENDIF line with no open block is also an error, so that the user
/// sees the mistake before the run. When the run gets to it, it does nothing.
fn check_if_blocks(lines: &ProgramLines) -> Result<(), String> {
    let mut open = Vec::new();
    for line in &lines.line_numbers {
        let items = &lines.items[line];
        if let Some(Item::Statement(Statement::EndIf)) = items.first() {
            if open.pop().is_none() {
                return Err(format!("ENDIF without matching IF{}", at_line(*line)));
            }
        }
        if let Some(Item::Statement(Statement::IfBlock { .. })) = items.last() {
            open.push(*line);
        }
    }
    match open.last() {
        Some(line) => Err(format!("IF without matching ENDIF{}", at_line(*line))),
        None => Ok(()),
    }
}

/// Define the DEF PROC and DEF FN statements of a line before the run, so
/// that a definition after END works.
fn define_routines(executor: &mut Executor, line_number: u16, items: &[Item]) {
    for (index, item) in items.iter().enumerate() {
        let position = Position {
            line: line_number.into(),
            index,
        };
        match item {
            Item::Statement(Statement::DefProc { name, params }) => {
                executor.define_procedure(name.clone(), position, params.clone());
            }
            Item::Statement(Statement::DefFn {
                name,
                params,
                expression,
            }) => {
                let body = match expression {
                    Some(expression) => FunctionBody::Expression(expression.clone()),
                    None => FunctionBody::Lines(position),
                };
                executor.define_function(name.clone(), params.clone(), body);
            }
            _ => {}
        }
    }
}

/// Make a call to a multi-line function run its body in `lines`. Returns the
/// record of a failed body run, for the run loop.
fn install_function_body_runner(
    executor: &mut Executor,
    lines: &Rc<ProgramLines>,
) -> Rc<RefCell<BodyFailure>> {
    let failure = Rc::new(RefCell::new(BodyFailure::default()));
    let runner_lines = Rc::clone(lines);
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
    failure
}

/// " at line N" for a stored line, and nothing for the typed line.
/// Go to a calculated line number. Return false if the value is not a line
/// number from 0 to 65535 or the line does not exist.
fn goto_calculated_line(program: &mut Cursor, target: i32) -> bool {
    u16::try_from(target).is_ok_and(|line_number| program.goto_line(line_number))
}

fn at_line(line_key: u32) -> String {
    if line_key == TYPED_LINE {
        String::new()
    } else {
        format!(" at line {}", line_key)
    }
}

/// The BBC BASIC text of an error: "<text> at line N", or
/// "Syntax error at line N: <message>". The typed line has no line number.
fn error_text(error: &BBCBasicError, line_key: u32) -> String {
    match error {
        BBCBasicError::SyntaxError { message, .. } if line_key != TYPED_LINE => {
            format!("Syntax error at line {}: {}", line_key, message)
        }
        BBCBasicError::SyntaxError { message, .. } => format!("Syntax error: {}", message),
        _ => format!("{}{}", error, at_line(line_key)),
    }
}

/// Set ERR, ERL and the text for REPORT to an error. ERL is 0 for the
/// typed line.
fn record_error(executor: &mut Executor, error: &BBCBasicError, line_key: u32) {
    let error_line = u16::try_from(line_key).unwrap_or(0);
    executor.set_last_error(
        error_number(error),
        error_line,
        error_text(error, TYPED_LINE),
    );
}

/// Run from `start` until the run stops. A BASIC error in a stored line goes
/// to the ON ERROR handler.
fn run_lines(
    executor: &mut Executor,
    lines: &ProgramLines,
    failure: &RefCell<BodyFailure>,
    start: Option<Position>,
) -> Result<RunOutcome, String> {
    let mut cursor = Cursor {
        lines,
        current: start,
    };
    let mut previous_line = None;
    let local_depth = executor.local_depth();

    while let Some(position) = cursor.current {
        let line_number = position.line;
        let item = lines
            .item(position)
            .ok_or_else(|| format!("Line {} not found", line_number))?;

        // TRACE: a new stored line starts at its first statement, or when
        // the run comes from a different line
        if position.index == 0 || previous_line != Some(line_number) {
            if let Some(stored_line) = position.stored_line() {
                executor.trace_line(stored_line);
            }
        }
        previous_line = Some(line_number);

        let error = match step(executor, &mut cursor, item, position) {
            Ok(Flow::Next) => {
                cursor.next();
                continue;
            }
            Ok(Flow::Jumped) => continue,
            Ok(Flow::Stop) => break,
            Ok(Flow::Quit) => return Ok(RunOutcome::Quit),
            Ok(Flow::FnReturn(_)) => {
                return Err(format!("= without FN call{}", at_line(line_number)));
            }
            Err(error) => error,
        };

        // An error in a function body: use the line of the body statement
        let body = std::mem::take(&mut *failure.borrow_mut());
        let error_line = body.line.unwrap_or(line_number);

        match (body.abort, error) {
            (Some(BodyAbort::Stop), _) => break,
            (Some(BodyAbort::Quit), _) => return Ok(RunOutcome::Quit),
            (Some(BodyAbort::Fatal(message)), _) | (None, StepError::Fatal(message)) => {
                return Err(message);
            }
            (None, StepError::Runtime(e)) => {
                // Set ERL, ERR and the text for REPORT, also when no
                // handler catches the error
                record_error(executor, &e, error_line);

                // Handle an error in a stored line with the ON ERROR handler
                // if one is set
                let handler = executor.get_error_handler();
                let (Some(handler_line), Ok(_)) = (handler, u16::try_from(error_line)) else {
                    return Err(error_text(&e, error_line));
                };

                if !cursor.goto_line(handler_line) {
                    return Err(format!(
                        "Error handler line {} not found (from error at line {})",
                        handler_line, error_line
                    ));
                }
                // As in BBC BASIC, the handler runs outside each PROC, FN,
                // GOSUB and loop that was open: it sees the values that
                // LOCAL statements hid, and a RETURN, ENDPROC, NEXT, UNTIL
                // or ENDWHILE in it gives an error
                executor.exit_local_scopes_to(local_depth);
                executor.clear_control_stacks();
            }
        }
    }

    Ok(RunOutcome::Ended)
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
            Ok(Flow::Quit) => return abort_program(failure, line_number, BodyAbort::Quit),
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
    line_number: u32,
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
        Item::Invalid(message) => return Err(StepError::Fatal(message.clone())),
    };

    match statement {
        Statement::IfBlock { condition } => {
            if executor.eval_integer(condition)? != 0 {
                return Ok(Flow::Next);
            }
            program
                .skip_block(line_number, true)
                .ok_or_else(|| format!("IF without matching ENDIF{}", at_line(line_number)))?;
            return Ok(Flow::Jumped);
        }
        // The end of a THEN block: continue after the matching ENDIF
        Statement::BlockElse => {
            program
                .skip_block(line_number, false)
                .ok_or_else(|| format!("ELSE without matching ENDIF{}", at_line(line_number)))?;
            return Ok(Flow::Jumped);
        }
        Statement::EndIf => return Ok(Flow::Next),
        _ => {}
    }

    executor.execute_statement(statement)?;

    match statement {
        Statement::End | Statement::Stop => Ok(Flow::Stop),
        Statement::Quit => Ok(Flow::Quit),
        Statement::Goto {
            line_number: target,
        } => {
            if !program.goto_line(*target) {
                return Err(BBCBasicError::NoSuchLine((*target).into()).into());
            }
            Ok(Flow::Jumped)
        }
        Statement::Gosub {
            line_number: target,
        } => {
            if !program.goto_line(*target) {
                return Err(BBCBasicError::NoSuchLine((*target).into()).into());
            }
            // Push the current position so RETURN can come back here. A
            // trapped error must not leave a return position.
            executor.push_gosub_return(position);
            Ok(Flow::Jumped)
        }
        Statement::GotoExpression { target } => {
            let target = executor.eval_integer(target)?;
            if !goto_calculated_line(program, target) {
                return Err(BBCBasicError::NoSuchLine(target).into());
            }
            Ok(Flow::Jumped)
        }
        Statement::GosubExpression { target } => {
            let target = executor.eval_integer(target)?;
            if !goto_calculated_line(program, target) {
                return Err(BBCBasicError::NoSuchLine(target).into());
            }
            executor.push_gosub_return(position);
            Ok(Flow::Jumped)
        }
        Statement::OnGoto {
            expression,
            targets,
            ..
        } => {
            // BBC BASIC uses 1-based indexing
            let index = executor.eval_integer(expression)?;

            if index >= 1 && (index as usize) <= targets.len() {
                let target = targets[(index - 1) as usize];
                if !program.goto_line(target) {
                    return Err(BBCBasicError::NoSuchLine(target.into()).into());
                }
                return Ok(Flow::Jumped);
            }
            program.goto_after_next_else(position)?;
            Ok(Flow::Jumped)
        }
        Statement::OnGosub {
            expression,
            targets,
            ..
        } => {
            // BBC BASIC uses 1-based indexing
            let index = executor.eval_integer(expression)?;

            if index >= 1 && (index as usize) <= targets.len() {
                let target = targets[(index - 1) as usize];
                if !program.goto_line(target) {
                    return Err(BBCBasicError::NoSuchLine(target.into()).into());
                }
                executor.push_gosub_return(position);
                return Ok(Flow::Jumped);
            }
            program.goto_after_next_else(position)?;
            Ok(Flow::Jumped)
        }
        Statement::Return { .. } => {
            let return_position = executor
                .pop_gosub_return()
                .map_err(|_| BBCBasicError::NoGosub)?;
            // Go back to the GOSUB, then continue with the statement after it
            program.goto(return_position);
            Ok(Flow::Next)
        }
        Statement::ProcCall { name, args } => {
            let proc = executor
                .get_procedure(name)
                .ok_or_else(|| BBCBasicError::NoSuchFnProc(format!("PROC{}", name)))?;

            if args.len() != proc.params.len() {
                return Err(BBCBasicError::Arguments.into());
            }

            // Clone procedure data before evaluating the arguments
            let proc_position = proc.position;
            let params = proc.params.clone();

            // Evaluate all arguments in the scope of the caller. An error
            // here changes no variable, and the PROC scope is not open.
            let values = executor.evaluate_arguments(&params, args)?;

            // Each parameter is local to the PROC: ENDPROC restores the
            // caller variable of the same name
            executor.enter_local_scope();
            if let Err(error) = executor.bind_parameters(&params, values) {
                // The PROC did not start: close its scope, so that the
                // ON ERROR handler runs in the scope of the caller
                executor.exit_local_scope()?;
                return Err(error.into());
            }

            executor.push_gosub_return(position);

            // Continue with the statement after DEF PROC
            program.goto(proc_position);
            Ok(Flow::Next)
        }
        Statement::EndProc => {
            executor
                .exit_local_scope()
                .map_err(|_| BBCBasicError::NoProc)?;

            let return_position = executor
                .pop_gosub_return()
                .map_err(|_| BBCBasicError::NoProc)?;
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
            let repeat_position = executor.check_until(condition)?;
            if let Some(repeat_position) = repeat_position {
                // Condition false: loop back to the statement after REPEAT
                program.goto(repeat_position);
            }
            Ok(Flow::Next)
        }
        Statement::While { condition } => {
            let entered = executor.push_while(position, condition)?;
            if entered.is_none() {
                // Condition false: scan forward to the matching ENDWHILE
                let mut depth = 1;
                while depth > 0 {
                    let Some(current) = program.next() else {
                        return Err(format!(
                            "WHILE without matching ENDWHILE{}",
                            at_line(line_number)
                        )
                        .into());
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
                .ok_or(BBCBasicError::NoWhile)?;
            let Some(Statement::While { condition }) = program.lines.statement(while_position)
            else {
                return Err(format!(
                    "Could not find WHILE statement at line {}",
                    while_position.line
                )
                .into());
            };
            let loop_position = executor.check_endwhile(condition)?;
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
        BBCBasicError::BadDim => 10,
        BBCBasicError::StringTooLong => 19,
        BBCBasicError::TooBig => 20,
        BBCBasicError::OnRange => 40,
        BBCBasicError::OutOfData => 42,
        BBCBasicError::NoFor => 32,
        BBCBasicError::NoRepeat => 43,
        BBCBasicError::NoWhile => 46,
        BBCBasicError::CantMatchFor => 33,
        BBCBasicError::NotLocal => 12,
        BBCBasicError::NoSuchLine(_) => 41,
        BBCBasicError::NoGosub => 38,
        BBCBasicError::Arguments => 31,
        BBCBasicError::NoProc => 13,
        BBCBasicError::NoSuchFnProc(_) => 29,
        BBCBasicError::ChannelNotOpen(_) => 222,
        BBCBasicError::BadCall => 30,
        BBCBasicError::NoSuchVariable(_) => 26,
        BBCBasicError::ArrayNotDimensioned(_) => 14,
        BBCBasicError::SyntaxError { .. } => 16,
        BBCBasicError::BadProgram => 254,
        BBCBasicError::IllegalFunction => 31,
        BBCBasicError::BadMode => 25,
        // Not a BBC BASIC error: BBC BASIC does not use 150 (issue #102)
        BBCBasicError::BadJson(_) => 150,
        _ => 255,
    }
}
