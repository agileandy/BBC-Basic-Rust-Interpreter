//! Program run loop for BBC BASIC
//!
//! Runs a stored program from its first line and handles the control flow
//! between lines (GOTO, GOSUB, loops, procedures and ON ERROR).

use std::collections::HashMap;

use crate::error::BBCBasicError;
use crate::executor::Executor;
use crate::parser::{parse_statement, Statement};
use crate::program::ProgramStore;

/// What the run loop does after a statement.
enum Flow {
    /// Continue with the next line
    Next,
    /// The statement set the current line
    Jumped,
    /// Stop the program
    Stop,
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

        statements.insert(line_number, statement);
    }

    program.start_execution();

    while let Some(line_number) = program.get_current_line() {
        let statement = statements
            .get(&line_number)
            .ok_or_else(|| format!("Line {} not found", line_number))?;

        match step(executor, program, &statements, statement, line_number) {
            Ok(Flow::Next) => {
                program.next_line();
            }
            Ok(Flow::Jumped) => {}
            Ok(Flow::Stop) => break,
            Err(StepError::Fatal(message)) => return Err(message),
            Err(StepError::Runtime(e)) => {
                // Handle the error with the ON ERROR handler if one is set
                let Some(handler_line) = executor.get_error_handler() else {
                    return Err(format!("Runtime error at line {}: {:?}", line_number, e));
                };

                // Set error information (ERL and ERR)
                executor.set_last_error(error_number(&e), line_number, format!("{:?}", e));

                if !program.goto_line(handler_line) {
                    return Err(format!(
                        "Error handler line {} not found (from error at line {})",
                        handler_line, line_number
                    ));
                }
            }
        }
    }

    program.stop_execution();
    Ok(())
}

/// Execute one statement and apply its control flow.
fn step(
    executor: &mut Executor,
    program: &mut ProgramStore,
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
