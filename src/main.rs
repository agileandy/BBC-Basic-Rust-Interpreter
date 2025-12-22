use bbc_basic_interpreter::{
    tokenizer::{tokenize, detokenize},
    parser::parse_statement,
    executor::Executor,
    program::ProgramStore,
};
use std::io::{self, Write};

fn main() {
    println!("BBC BASIC Interpreter v0.1.0");
    println!("Type 'EXIT' to quit, 'HELP' for help\n");

    let mut executor = Executor::new();
    let mut program = ProgramStore::new();
    let stdin = io::stdin();
    let mut line_buffer = String::new();

    loop {
        // Prompt
        print!("> ");
        io::stdout().flush().unwrap();

        // Read line
        line_buffer.clear();
        if stdin.read_line(&mut line_buffer).is_err() {
            break;
        }

        let input = line_buffer.trim();

        // Check for commands
        if input.eq_ignore_ascii_case("exit") || input.eq_ignore_ascii_case("quit") {
            println!("Goodbye!");
            break;
        }

        if input.eq_ignore_ascii_case("help") {
            print_help();
            continue;
        }

        if input.is_empty() {
            continue;
        }

        // Handle special commands
        if input.eq_ignore_ascii_case("run") {
            match run_program(&mut executor, &mut program) {
                Ok(()) => {},
                Err(e) => println!("Error: {}", e),
            }
            continue;
        }

        if input.eq_ignore_ascii_case("list") {
            list_program(&program);
            continue;
        }

        if input.eq_ignore_ascii_case("new") {
            program.clear();
            println!("Program cleared");
            continue;
        }

        // Process the line (either store or execute)
        match process_line(&mut executor, &mut program, input) {
            Ok(()) => {},
            Err(e) => println!("Error: {}", e),
        }
    }
}

fn process_line(executor: &mut Executor, program: &mut ProgramStore, line: &str) -> Result<(), String> {
    // Tokenize
    let tokenized = tokenize(line)
        .map_err(|e| format!("Tokenization error: {:?}", e))?;

    // Check if this is a numbered line (program mode) or immediate mode
    if let Some(line_number) = tokenized.line_number {
        // Program mode: store the line
        if tokenized.tokens.is_empty() {
            // Just a line number with no statement = delete that line
            program.delete_line(line_number);
            println!("Line {} deleted", line_number);
        } else {
            program.store_line(tokenized);
            // Silent storage (like real BBC BASIC)
        }
        Ok(())
    } else {
        // Immediate mode: execute immediately
        let statement = parse_statement(&tokenized)
            .map_err(|e| format!("Parse error: {:?}", e))?;

        executor.execute_statement(&statement)
            .map_err(|e| format!("Runtime error: {:?}", e))?;

        Ok(())
    }
}

fn run_program(executor: &mut Executor, program: &mut ProgramStore) -> Result<(), String> {
    if program.is_empty() {
        return Err("No program to run".to_string());
    }
    
    // CRITICAL: Reset and collect all DATA statements BEFORE execution begins
    // This ensures READ can access DATA regardless of program flow (GOTO, etc.)
    executor.reset_data();
    
    // First pass: collect all DATA statements
    for (line_number, line) in program.list() {
        let statement = parse_statement(line)
            .map_err(|e| format!("Parse error at line {}: {:?}", line_number, e))?;
        
        // Only collect DATA statements, don't execute anything yet
        if matches!(statement, bbc_basic_interpreter::Statement::Data { .. }) {
            executor.collect_data(&statement)
                .map_err(|e| format!("Error collecting DATA at line {}: {:?}", line_number, e))?;
        }
    }

    // Start execution from first line
    program.start_execution();

    while let Some(line_number) = program.get_current_line() {
        // Get the line
        let line = program.get_line(line_number)
            .ok_or_else(|| format!("Line {} not found", line_number))?;

        // Parse the statement
        let statement = parse_statement(line)
            .map_err(|e| format!("Parse error at line {}: {:?}", line_number, e))?;

        // Check statement type before executing
        let is_goto = matches!(statement, bbc_basic_interpreter::Statement::Goto { .. });
        let is_gosub = matches!(statement, bbc_basic_interpreter::Statement::Gosub { .. });
        let is_return = matches!(statement, bbc_basic_interpreter::Statement::Return);
        let is_end = matches!(statement, bbc_basic_interpreter::Statement::End | bbc_basic_interpreter::Statement::Stop);
        let is_for = matches!(statement, bbc_basic_interpreter::Statement::For { .. });
        let is_next = matches!(statement, bbc_basic_interpreter::Statement::Next { .. });
        let is_repeat = matches!(statement, bbc_basic_interpreter::Statement::Repeat);
        let is_until = matches!(statement, bbc_basic_interpreter::Statement::Until { .. });

        // Execute the statement
        executor.execute_statement(&statement)
            .map_err(|e| format!("Runtime error at line {}: {:?}", line_number, e))?;

        // Handle control flow
        if is_end {
            break;
        } else if is_goto {
            // GOTO: extract target and jump
            if let bbc_basic_interpreter::Statement::Goto { line_number: target } = statement {
                if !program.goto_line(target) {
                    return Err(format!("Line {} not found (GOTO)", target));
                }
            }
        } else if is_gosub {
            // GOSUB: save return address (this line) and jump to target
            if let bbc_basic_interpreter::Statement::Gosub { line_number: target } = statement {
                // Push the current line number so RETURN can come back here
                executor.push_gosub_return(line_number);
                
                // Jump to the target subroutine
                if !program.goto_line(target) {
                    return Err(format!("Line {} not found (GOSUB)", target));
                }
            }
        } else if is_return {
            // RETURN: pop return address and jump back
            match executor.pop_gosub_return() {
                Ok(return_line) => {
                    // Jump back to the line that called GOSUB
                    if program.goto_line(return_line) {
                        // Move to the line AFTER the GOSUB
                        program.next_line();
                    } else {
                        return Err(format!("Return line {} not found", return_line));
                    }
                }
                Err(_) => {
                    return Err("RETURN without GOSUB".to_string());
                }
            }
        } else if is_for {
            // FOR: record this line number for NEXT to loop back to
            executor.set_for_loop_line(line_number);
            program.next_line();
        } else if is_next {
            // NEXT: check if we should loop back
            if let Some(for_line) = executor.should_loop_back() {
                // Loop continues - go back to the line AFTER the FOR statement
                if program.goto_line(for_line) {
                    program.next_line(); // Move to line after FOR
                } else {
                    return Err(format!("FOR loop line {} not found", for_line));
                }
            } else {
                // Loop completed - continue to next line
                program.next_line();
            }
        } else if is_repeat {
            // REPEAT: push this line number for UNTIL to loop back to
            executor.push_repeat(line_number);
            program.next_line();
        } else if is_until {
            // UNTIL: check condition and loop back if false
            if let bbc_basic_interpreter::Statement::Until { condition } = statement {
                match executor.check_until(&condition) {
                    Ok(Some(repeat_line)) => {
                        // Condition false - loop back to line AFTER REPEAT
                        if program.goto_line(repeat_line) {
                            program.next_line();
                        } else {
                            return Err(format!("REPEAT line {} not found", repeat_line));
                        }
                    }
                    Ok(None) => {
                        // Condition true - exit loop, continue to next line
                        program.next_line();
                    }
                    Err(e) => {
                        return Err(format!("Error evaluating UNTIL condition: {:?}", e));
                    }
                }
            }
        } else {
            // Normal: advance to next line
            if program.next_line().is_none() {
                break;
            }
        }
    }

    program.stop_execution();
    Ok(())
}

fn list_program(program: &ProgramStore) {
    if program.is_empty() {
        println!("No program");
        return;
    }

    for (line_number, line) in program.list() {
        match detokenize(line) {
            Ok(text) => println!("{}", text),
            Err(e) => println!("Error listing line {}: {:?}", line_number, e),
        }
    }
}

fn print_help() {
    println!("BBC BASIC Interpreter - Available Commands:");
    println!();
    println!("Program Mode (with line numbers):");
    println!("  10 PRINT \"HELLO\"        - Store program line");
    println!("  20 GOTO 10               - Store line with GOTO");
    println!("  10                       - Delete line 10");
    println!("  LIST                     - List the program");
    println!("  RUN                      - Run the stored program");
    println!("  NEW                      - Clear the program");
    println!();
    println!("Immediate Mode (no line numbers):");
    println!("  A% = 42                  - Execute immediately");
    println!("  PRINT \"text\"             - Execute immediately");
    println!();
    println!("Statements:");
    println!("  LET A% = 42              - Assign integer variable");
    println!("  B = 3.14                 - Assign real variable (LET optional)");
    println!("  C$ = \"HELLO\"             - Assign string variable");
    println!("  PRINT \"text\", A%, B      - Print values");
    println!("  FOR I% = 1 TO 10         - Start FOR loop");
    println!("  NEXT I%                  - End FOR loop");
    println!("  INPUT A%, B$             - Input variables");
    println!("  DIM A%(10)               - Dimension array");
    println!("  GOTO 100                 - Jump to line");
    println!("  GOSUB 1000               - Call subroutine");
    println!("  RETURN                   - Return from subroutine");
    println!("  REM comment              - Comment");
    println!("  END                      - End program");
    println!();
    println!("Examples:");
    println!("  10 PRINT \"Hello\"");
    println!("  20 GOTO 10");
    println!("  LIST");
    println!("  RUN");
    println!();
    println!("Variable Types:");
    println!("  A%  - Integer variable");
    println!("  B   - Real (float) variable");
    println!("  C$  - String variable");
    println!();
}
