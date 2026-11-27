use bbc_basic_interpreter::{
    executor::Executor,
    program::ProgramStore,
    runner::{run_program_with_outcome, run_typed_line, RunOutcome},
    tokenizer::{detokenize, tokenize},
};
use std::io::{self, Write};

/// True if a panic message comes from a write to a closed output pipe.
fn is_closed_output_panic(message: &str) -> bool {
    message.contains("Broken pipe") || message.contains("BrokenPipe")
}

/// End the process with status 0 when the output closes early.
/// Show all other panics as before, with a status that is not 0.
fn install_closed_output_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if info.payload_as_str().is_some_and(is_closed_output_panic) {
            std::process::exit(0);
        }
        default_hook(info);
    }));
}

fn main() {
    install_closed_output_hook();

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
        // Ok(0) is end of input. Stop for it and for an error.
        if matches!(stdin.read_line(&mut line_buffer), Ok(0) | Err(_)) {
            println!();
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
            match run_program_with_outcome(&mut executor, &mut program) {
                Ok(RunOutcome::Ended) => {}
                Ok(RunOutcome::Quit) => {
                    println!("Goodbye!");
                    break;
                }
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
                    Ok(_) => match run_program_with_outcome(&mut executor, &mut program) {
                        Ok(RunOutcome::Ended) => {}
                        Ok(RunOutcome::Quit) => {
                            println!("Goodbye!");
                            break;
                        }
                        Err(e) => println!("Error: {}", e),
                    },
                    Err(e) => println!("Error: {}", e),
                },
                Err(e) => println!("Error: {}", e),
            }
            continue;
        }

        // *CAT command (catalog files)
        if let Some(directory) = cat_directory(input) {
            if let Err(e) = catalog_files(directory) {
                println!("Error: {}", e);
            }
            continue;
        }

        if let Some(range) = delete_command(input) {
            match range {
                Ok((start, end)) => program.delete_range(start, end),
                Err(e) => println!("Error: {}", e),
            }
            continue;
        }

        // Process the line (either store or execute)
        match process_line(&mut executor, &mut program, input) {
            Ok(RunOutcome::Ended) => {}
            Ok(RunOutcome::Quit) => {
                println!("Goodbye!");
                break;
            }
            Err(e) => println!("Error: {}", e),
        }
    }
}

fn process_line(
    executor: &mut Executor,
    program: &mut ProgramStore,
    line: &str,
) -> Result<RunOutcome, String> {
    // Tokenize
    let tokenized = tokenize(line).map_err(|e| format!("Tokenization error: {:?}", e))?;

    // Check if this is a numbered line (program mode) or immediate mode
    if let Some(line_number) = tokenized.line_number {
        // Program mode: store the line
        if tokenized.tokens.is_empty() {
            // Just a line number with no statement = delete that line.
            // Silent, like real BBC BASIC, also when the line does not exist.
            program.delete_line(line_number);
        } else {
            program.store_line(tokenized);
            // Silent storage (like real BBC BASIC)
        }
        Ok(RunOutcome::Ended)
    } else {
        // Immediate mode: run the line with the control flow of a program line
        run_typed_line(executor, program, &tokenized)
    }
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

/// Use the file name as given if it has an extension, else add .bbas
fn resolve_program_path(filename: &str) -> String {
    if std::path::Path::new(filename).extension().is_some() {
        filename.to_string()
    } else {
        format!("{}.bbas", filename)
    }
}

/// Save current program to a file (.bbas added if no extension)
fn save_program(program: &ProgramStore, filename: &str) -> Result<(), String> {
    if program.is_empty() {
        return Err("No program to save".to_string());
    }

    let path = resolve_program_path(filename);

    // Open file for writing
    let mut file = std::fs::File::create(&path)
        .map_err(|e| format!("Failed to create file {}: {}", path, e))?;

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

/// Load program from a file (.bbas added if no extension)
fn load_program(program: &mut ProgramStore, filename: &str) -> Result<(), String> {
    let path = resolve_program_path(filename);

    // Read file
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read file {}: {}", path, e))?;

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

/// Match `DELETE start, end` (any case). Return the range, or a syntax error.
fn delete_command(input: &str) -> Option<Result<(u16, u16), String>> {
    if !input.get(..6)?.eq_ignore_ascii_case("delete") {
        return None;
    }
    let rest = &input[6..];
    if rest.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
        return None;
    }
    let numbers: Vec<Option<u16>> = rest.split(',').map(|n| n.trim().parse().ok()).collect();
    Some(match numbers[..] {
        [Some(start), Some(end)] if start <= end => Ok((start, end)),
        [Some(_), Some(_)] => Err("Syntax error: DELETE start is more than end".to_string()),
        _ => Err("Syntax error: use DELETE start, end".to_string()),
    })
}

/// Match `*CAT` or `*CAT <directory>` (any case). Return the directory, or "" for none.
fn cat_directory(input: &str) -> Option<&str> {
    let input = input.trim();
    if !input.get(..4)?.eq_ignore_ascii_case("*cat") {
        return None;
    }
    let rest = &input[4..];
    if rest.is_empty() {
        return Some("");
    }
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let rest = rest.trim();
    let rest = rest
        .strip_prefix('"')
        .and_then(|r| r.strip_suffix('"'))
        .unwrap_or(rest);
    Some(rest)
}

/// Catalog all .bbas and .bas files in the directory ("" = current directory)
fn catalog_files(directory: &str) -> Result<(), String> {
    let path = if directory.is_empty() { "." } else { directory };
    if !std::path::Path::new(path).is_dir() {
        return Err(format!("Directory not found: {}", path));
    }
    let paths = std::fs::read_dir(path).map_err(|e| format!("Failed to read directory: {}", e))?;

    if directory.is_empty() {
        println!("\nCatalog:");
    } else {
        println!("\nCatalog: {}", directory);
    }
    println!("{:<30} {:>10}  Modified", "Filename", "Size");
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

        if filename_str.ends_with(".bbas") || filename_str.ends_with(".bas") {
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
        println!("(no .bbas or .bas files found)");
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
    println!("  DELETE 20, 40            - Delete lines 20 to 40");
    println!("  SAVE \"filename\"          - Save program to filename.bbas");
    println!("  LOAD \"filename\"          - Load program from filename.bbas");
    println!("  CHAIN \"filename\"         - Load and run program");
    println!("  *CAT                     - List all .bbas and .bas files");
    println!("  *CAT directory           - List the .bbas and .bas files in directory");
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

#[cfg(test)]
mod tests {
    use super::is_closed_output_panic;

    #[test]
    fn test_print_to_closed_stdout_is_closed_output() {
        assert!(is_closed_output_panic(
            "failed printing to stdout: Broken pipe (os error 32)"
        ));
    }

    #[test]
    fn test_unwrap_of_broken_pipe_error_is_closed_output() {
        assert!(is_closed_output_panic(
            "called `Result::unwrap()` on an `Err` value: Os { code: 32, kind: BrokenPipe, message: \"Broken pipe\" }"
        ));
    }

    #[test]
    fn test_other_panic_is_not_closed_output() {
        assert!(!is_closed_output_panic("index out of bounds"));
        assert!(!is_closed_output_panic(
            "failed printing to stdout: Permission denied (os error 13)"
        ));
    }
}
