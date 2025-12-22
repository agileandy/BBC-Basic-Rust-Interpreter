use bbc_basic_interpreter::{
    executor::Executor,
    parser::parse_statement,
    program::ProgramStore,
    tokenizer::{detokenize, tokenize},
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
                Ok(()) => {}
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

        // SAVE command
        let input_upper = input.to_uppercase();
        if input_upper.starts_with("SAVE ") {
            match extract_filename(input) {
                Ok(filename) => {
                    if let Err(e) = save_program(&program, &filename) {
                        println!("Error: {}", e);
                    }
                }
                Err(e) => println!("Error: {}", e),
            }
            continue;
        }

        // LOAD command
        if input_upper.starts_with("LOAD ") {
            match extract_filename(input) {
                Ok(filename) => {
                    if let Err(e) = load_program(&mut program, &filename) {
                        println!("Error: {}", e);
                    }
                }
                Err(e) => println!("Error: {}", e),
            }
            continue;
        }

        // CHAIN command (LOAD and RUN)
        if input_upper.starts_with("CHAIN ") {
            match extract_filename(input) {
                Ok(filename) => match load_program(&mut program, &filename) {
                    Ok(_) => {
                        if let Err(e) = run_program(&mut executor, &mut program) {
                            println!("Error: {}", e);
                        }
                    }
                    Err(e) => println!("Error: {}", e),
                },
                Err(e) => println!("Error: {}", e),
            }
            continue;
        }

        // *CAT command (catalog files)
        if input.trim() == "*CAT" || input.trim().eq_ignore_ascii_case("*cat") {
            if let Err(e) = catalog_files() {
                println!("Error: {}", e);
            }
            continue;
        }

        // Process the line (either store or execute)
        match process_line(&mut executor, &mut program, input) {
            Ok(()) => {}
            Err(e) => println!("Error: {}", e),
        }
    }
}

fn process_line(
    executor: &mut Executor,
    program: &mut ProgramStore,
    line: &str,
) -> Result<(), String> {
    // Tokenize
    let tokenized = tokenize(line).map_err(|e| format!("Tokenization error: {:?}", e))?;

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
        let statement = parse_statement(&tokenized).map_err(|e| format!("Parse error: {:?}", e))?;

        executor
            .execute_statement(&statement)
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

    // First pass: collect all DATA statements and procedure definitions
    executor.clear_procedures();
    for (line_number, line) in program.list() {
        let statement = parse_statement(line)
            .map_err(|e| format!("Parse error at line {}: {:?}", line_number, e))?;

        // Collect DATA statements
        if matches!(statement, bbc_basic_interpreter::Statement::Data { .. }) {
            executor
                .collect_data(&statement)
                .map_err(|e| format!("Error collecting DATA at line {}: {:?}", line_number, e))?;
        }

        // Collect procedure definitions
        if let bbc_basic_interpreter::Statement::DefProc { name, params } = statement {
            executor.define_procedure(name, line_number, params);
        }
    }

    // Start execution from first line
    program.start_execution();

    while let Some(line_number) = program.get_current_line() {
        // Get the line
        let line = program
            .get_line(line_number)
            .ok_or_else(|| format!("Line {} not found", line_number))?;

        // Parse the statement
        let statement = parse_statement(line)
            .map_err(|e| format!("Parse error at line {}: {:?}", line_number, e))?;

        // Check statement type before executing
        let is_goto = matches!(statement, bbc_basic_interpreter::Statement::Goto { .. });
        let is_gosub = matches!(statement, bbc_basic_interpreter::Statement::Gosub { .. });
        let is_on_goto = matches!(statement, bbc_basic_interpreter::Statement::OnGoto { .. });
        let is_on_gosub = matches!(statement, bbc_basic_interpreter::Statement::OnGosub { .. });
        let is_return = matches!(statement, bbc_basic_interpreter::Statement::Return);
        let is_end = matches!(
            statement,
            bbc_basic_interpreter::Statement::End | bbc_basic_interpreter::Statement::Stop
        );
        let is_for = matches!(statement, bbc_basic_interpreter::Statement::For { .. });
        let is_next = matches!(statement, bbc_basic_interpreter::Statement::Next { .. });
        let is_repeat = matches!(statement, bbc_basic_interpreter::Statement::Repeat);
        let is_until = matches!(statement, bbc_basic_interpreter::Statement::Until { .. });
        let is_while = matches!(statement, bbc_basic_interpreter::Statement::While { .. });
        let is_endwhile = matches!(statement, bbc_basic_interpreter::Statement::EndWhile);
        let is_proc_call = matches!(statement, bbc_basic_interpreter::Statement::ProcCall { .. });
        let is_endproc = matches!(statement, bbc_basic_interpreter::Statement::EndProc);

        // Execute the statement
        let execution_result = executor.execute_statement(&statement);

        // Handle errors with ON ERROR handler if set
        if let Err(e) = execution_result {
            if let Some(handler_line) = executor.get_error_handler() {
                // Convert BBCBasicError to error number
                let error_number = match &e {
                    bbc_basic_interpreter::BBCBasicError::DivisionByZero => 18,
                    bbc_basic_interpreter::BBCBasicError::TypeMismatch => 6,
                    bbc_basic_interpreter::BBCBasicError::SubscriptOutOfRange => 15,
                    bbc_basic_interpreter::BBCBasicError::NoRoom => 11,
                    bbc_basic_interpreter::BBCBasicError::StringTooLong => 19,
                    bbc_basic_interpreter::BBCBasicError::NoSuchVariable(_) => 26,
                    bbc_basic_interpreter::BBCBasicError::ArrayNotDimensioned(_) => 14,
                    bbc_basic_interpreter::BBCBasicError::SyntaxError { .. } => 220,
                    bbc_basic_interpreter::BBCBasicError::BadProgram => 254,
                    bbc_basic_interpreter::BBCBasicError::IllegalFunction => 31,
                    _ => 255, // Unknown error
                };

                // Set error information (ERL and ERR)
                executor.set_last_error(error_number, line_number, format!("{:?}", e));

                // Jump to error handler
                if !program.goto_line(handler_line) {
                    return Err(format!(
                        "Error handler line {} not found (from error at line {})",
                        handler_line, line_number
                    ));
                }
                // Continue execution from error handler
                continue;
            } else {
                // No error handler - propagate error as before
                return Err(format!("Runtime error at line {}: {:?}", line_number, e));
            }
        }

        // Handle control flow
        if is_end {
            break;
        } else if is_goto {
            // GOTO: extract target and jump
            if let bbc_basic_interpreter::Statement::Goto {
                line_number: target,
            } = statement
            {
                if !program.goto_line(target) {
                    return Err(format!("Line {} not found (GOTO)", target));
                }
            }
        } else if is_gosub {
            // GOSUB: save return address (this line) and jump to target
            if let bbc_basic_interpreter::Statement::Gosub {
                line_number: target,
            } = statement
            {
                // Push the current line number so RETURN can come back here
                executor.push_gosub_return(line_number);

                // Jump to the target subroutine
                if !program.goto_line(target) {
                    return Err(format!("Line {} not found (GOSUB)", target));
                }
            }
        } else if is_on_goto {
            // ON GOTO: evaluate expression and jump to computed target
            if let bbc_basic_interpreter::Statement::OnGoto {
                expression,
                targets,
            } = &statement
            {
                // Evaluate expression - BBC BASIC uses 1-based indexing
                let index = executor
                    .eval_integer(expression)
                    .map_err(|e| format!("Error evaluating ON GOTO expression: {:?}", e))?;

                // Check if index is valid (1-based, so 1 = first target, 2 = second, etc.)
                if index >= 1 && (index as usize) <= targets.len() {
                    let target = targets[(index - 1) as usize];
                    if !program.goto_line(target) {
                        return Err(format!("Line {} not found (ON GOTO)", target));
                    }
                }
                // If index is out of range, just continue to next line (fall through)
            }
        } else if is_on_gosub {
            // ON GOSUB: evaluate expression and gosub to computed target
            if let bbc_basic_interpreter::Statement::OnGosub {
                expression,
                targets,
            } = &statement
            {
                // Evaluate expression - BBC BASIC uses 1-based indexing
                let index = executor
                    .eval_integer(expression)
                    .map_err(|e| format!("Error evaluating ON GOSUB expression: {:?}", e))?;

                // Check if index is valid (1-based)
                if index >= 1 && (index as usize) <= targets.len() {
                    let target = targets[(index - 1) as usize];

                    // Push return address
                    executor.push_gosub_return(line_number);

                    // Jump to target
                    if !program.goto_line(target) {
                        return Err(format!("Line {} not found (ON GOSUB)", target));
                    }
                }
                // If index is out of range, just continue to next line (fall through)
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
        } else if is_proc_call {
            // PROC call: get procedure definition, bind parameters, push return address, jump
            if let bbc_basic_interpreter::Statement::ProcCall { name, args } = statement {
                // Get procedure definition
                let proc = executor
                    .get_procedure(&name)
                    .ok_or_else(|| format!("Procedure {} not defined", name))?;

                // Check parameter count
                if args.len() != proc.params.len() {
                    return Err(format!(
                        "Procedure {} expects {} parameters, got {}",
                        name,
                        proc.params.len(),
                        args.len()
                    ));
                }

                // Clone procedure data before entering local scope
                let proc_line = proc.line_number;
                let params_and_args: Vec<_> = proc
                    .params
                    .iter()
                    .zip(args.iter())
                    .map(|(p, a)| (p.clone(), a.clone()))
                    .collect();

                // Enter local scope for procedure
                executor.enter_local_scope();

                // Bind arguments to parameters (as global variables)
                for (param_name, arg_expr) in params_and_args {
                    executor
                        .execute_statement(&bbc_basic_interpreter::Statement::Assignment {
                            target: param_name,
                            expression: arg_expr,
                        })
                        .map_err(|e| format!("Error binding parameter: {:?}", e))?;
                }

                // Push return address (current line number)
                executor.push_gosub_return(line_number);

                // Jump to procedure line
                if !program.goto_line(proc_line) {
                    return Err(format!("Procedure {} line {} not found", name, proc_line));
                }

                // Move to line AFTER DEF PROC (skip the definition line)
                program.next_line();
            }
        } else if is_endproc {
            // ENDPROC: exit local scope and pop return address
            executor
                .exit_local_scope()
                .map_err(|e| format!("Error exiting local scope: {:?}", e))?;

            match executor.pop_gosub_return() {
                Ok(return_line) => {
                    // Jump back to the line that called PROC
                    if program.goto_line(return_line) {
                        // Move to the line AFTER the PROC call
                        program.next_line();
                    } else {
                        return Err(format!("Return line {} not found", return_line));
                    }
                }
                Err(_) => {
                    return Err("ENDPROC without PROC call".to_string());
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
        } else if is_while {
            // WHILE: check condition and enter loop if true, skip to ENDWHILE if false
            if let bbc_basic_interpreter::Statement::While { condition } = statement {
                match executor.push_while(line_number, &condition) {
                    Ok(Some(_)) => {
                        // Condition true - enter loop body
                        program.next_line();
                    }
                    Ok(None) => {
                        // Condition false - skip to line after ENDWHILE
                        // Find the matching ENDWHILE by scanning forward
                        let mut depth = 1;
                        while depth > 0 {
                            if program.next_line().is_none() {
                                return Err("WHILE without matching ENDWHILE".to_string());
                            }
                            
                            let current_line = program.get_current_line().unwrap();
                            if let Some(line) = program.get_line(current_line) {
                                if let Ok(stmt) = parse_statement(&line) {
                                    if matches!(stmt, bbc_basic_interpreter::Statement::While { .. }) {
                                        depth += 1;
                                    } else if matches!(stmt, bbc_basic_interpreter::Statement::EndWhile) {
                                        depth -= 1;
                                    }
                                }
                            }
                        }
                        program.next_line(); // Move past ENDWHILE
                    }
                    Err(e) => {
                        return Err(format!("Error evaluating WHILE condition: {:?}", e));
                    }
                }
            }
        } else if is_endwhile {
            // ENDWHILE: check condition and loop back if true
            // Need to retrieve the WHILE condition from the original WHILE statement
            // Find the matching WHILE by using the while_stack
            if let Some(while_line) = executor.check_endwhile_get_while_line() {
                if let Some(line) = program.get_line(while_line) {
                    if let Ok(bbc_basic_interpreter::Statement::While { condition }) = 
                        parse_statement(&line) {
                        match executor.check_endwhile(&condition) {
                            Ok(Some(while_line_num)) => {
                                // Condition still true - loop back to line AFTER WHILE
                                if program.goto_line(while_line_num) {
                                    program.next_line();
                                } else {
                                    return Err(format!("WHILE line {} not found", while_line_num));
                                }
                            }
                            Ok(None) => {
                                // Condition false - exit loop, continue to next line
                                program.next_line();
                            }
                            Err(e) => {
                                return Err(format!("Error evaluating WHILE condition at ENDWHILE: {:?}", e));
                            }
                        }
                    } else {
                        return Err(format!("Could not parse WHILE statement at line {}", while_line));
                    }
                } else {
                    return Err(format!("WHILE line {} not found", while_line));
                }
            } else {
                return Err("ENDWHILE without matching WHILE".to_string());
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

/// Extract filename from command like SAVE "filename" or LOAD "filename"
fn extract_filename(input: &str) -> Result<String, String> {
    // Split on first space to get command and rest
    let parts: Vec<&str> = input.splitn(2, ' ').collect();
    if parts.len() < 2 {
        return Err("Expected filename".to_string());
    }

    let filename = parts[1].trim();

    // Remove quotes if present
    let filename = if filename.starts_with('"') && filename.ends_with('"') {
        &filename[1..filename.len() - 1]
    } else {
        filename
    };

    if filename.is_empty() {
        return Err("Filename cannot be empty".to_string());
    }

    Ok(filename.to_string())
}

/// Save current program to a .bbas file
fn save_program(program: &ProgramStore, filename: &str) -> Result<(), String> {
    if program.is_empty() {
        return Err("No program to save".to_string());
    }

    // Add .bbas extension if not present
    let path = if filename.ends_with(".bbas") {
        filename.to_string()
    } else {
        format!("{}.bbas", filename)
    };

    // Open file for writing
    let mut file =
        std::fs::File::create(&path).map_err(|e| format!("Failed to create file: {}", e))?;

    // Write each line (detokenized)
    use std::io::Write;
    for (line_number, line) in program.list() {
        let text = detokenize(line)
            .map_err(|e| format!("Failed to detokenize line {}: {:?}", line_number, e))?;
        writeln!(file, "{}", text)
            .map_err(|e| format!("Failed to write line {}: {}", line_number, e))?;
    }

    println!("Saved to {}", path);
    Ok(())
}

/// Load program from a .bbas file
fn load_program(program: &mut ProgramStore, filename: &str) -> Result<(), String> {
    // Add .bbas extension if not present
    let path = if filename.ends_with(".bbas") {
        filename.to_string()
    } else {
        format!("{}.bbas", filename)
    };

    // Read file
    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e))?;

    // Clear current program (like NEW command)
    program.clear();
    // Note: We don't reset executor state - variables persist across LOAD
    // This matches BBC BASIC behavior where LOAD doesn't clear variables

    // Parse and add each line
    for (line_num, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue; // Skip empty lines
        }

        // Tokenize and store
        let tokenized =
            tokenize(line).map_err(|e| format!("Parse error at line {}: {:?}", line_num + 1, e))?;

        if tokenized.line_number.is_some() {
            program.store_line(tokenized);
        } else {
            return Err(format!(
                "Line {} has no line number: {}",
                line_num + 1,
                line
            ));
        }
    }

    println!("Loaded from {}", path);
    Ok(())
}

/// Catalog all .bbas files in current directory
fn catalog_files() -> Result<(), String> {
    let paths = std::fs::read_dir(".").map_err(|e| format!("Failed to read directory: {}", e))?;

    println!("\nCatalog:");
    println!("{:<30} {:>10}  {}", "Filename", "Size", "Modified");
    println!("{}", "-".repeat(60));

    let mut count = 0;
    let mut entries: Vec<_> = paths.collect();
    entries.sort_by_key(|e| {
        e.as_ref()
            .ok()
            .and_then(|e| e.file_name().to_str().map(|s| s.to_lowercase()))
    });

    for path in entries {
        let path = path.map_err(|e| format!("Failed to read entry: {}", e))?;
        let filename = path.file_name();
        let filename_str = filename.to_string_lossy();

        if filename_str.ends_with(".bbas") {
            let metadata = path
                .metadata()
                .map_err(|e| format!("Failed to read metadata: {}", e))?;

            let size = metadata.len();
            let modified = metadata
                .modified()
                .ok()
                .and_then(|m| m.elapsed().ok())
                .map(|d| {
                    let secs = d.as_secs();
                    if secs < 60 {
                        format!("{}s ago", secs)
                    } else if secs < 3600 {
                        format!("{}m ago", secs / 60)
                    } else if secs < 86400 {
                        format!("{}h ago", secs / 3600)
                    } else {
                        format!("{}d ago", secs / 86400)
                    }
                })
                .unwrap_or_else(|| "unknown".to_string());

            println!("{:<30} {:>10}  {}", filename_str, size, modified);
            count += 1;
        }
    }

    if count == 0 {
        println!("(no .bbas files found)");
    } else {
        println!("\n{} file(s)", count);
    }

    Ok(())
}

fn print_help() {
    println!("BBC BASIC Interpreter - Available Commands:");
    println!();
    println!("Program Mode (with line numbers):");
    println!("  10 PRINT \"HELLO\"        - Store program line");
    println!("  20 GOTO 10               - Store line with GOTO");
    println!("  10                       - Delete line 10");
    println!();
    println!("Immediate Commands:");
    println!("  LIST                     - List the program");
    println!("  RUN                      - Run the stored program");
    println!("  NEW                      - Clear the program");
    println!("  SAVE \"filename\"          - Save program to filename.bbas");
    println!("  LOAD \"filename\"          - Load program from filename.bbas");
    println!("  CHAIN \"filename\"         - Load and run program");
    println!("  *CAT                     - List all .bbas files");
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
    println!("  DEF PROC name(params)    - Define procedure");
    println!("  PROC name(args)          - Call procedure");
    println!("  ENDPROC                  - End procedure");
    println!("  REM comment              - Comment");
    println!("  END                      - End program");
    println!();
    println!("Examples:");
    println!("  10 PRINT \"Hello\"");
    println!("  20 GOTO 10");
    println!("  LIST");
    println!("  RUN");
    println!("  SAVE \"myprog\"");
    println!("  LOAD \"myprog\"");
    println!();
    println!("Variable Types:");
    println!("  A%  - Integer variable");
    println!("  B   - Real (float) variable");
    println!("  C$  - String variable");
    println!();
}
