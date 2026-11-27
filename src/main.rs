use bbc_basic_interpreter::{
    executor::Executor,
    program::{parse_list_range, ProgramStore},
    runner::{run_program_with_outcome, run_typed_line, RunOutcome},
    tokenizer::{detokenize, tokenize},
    ui,
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

/// The native stack size of the interpreter thread. A nested FN call can use
/// about 25 KiB of it in a debug build. The main thread has 8 MiB, which
/// overflows before the limits of the executor and the parser (issue 151).
const INTERPRETER_STACK_SIZE: usize = 64 * 1024 * 1024;

fn main() {
    install_closed_output_hook();

    // `--ide [file]` starts the IDE. No argument starts the command prompt.
    match ui::parse_args(std::env::args().skip(1)) {
        Ok(ui::Start::Prompt) => {}
        Ok(ui::Start::Ide(file_name)) => std::process::exit(ui::main(file_name)),
        Err(usage) => {
            eprintln!("{}", usage);
            std::process::exit(2);
        }
    }

    if let Err(error) = run_on_thread(INTERPRETER_STACK_SIZE, run_interpreter) {
        // Deep calls can then overflow the smaller stack of the main thread
        eprintln!("Warning: no thread with a 64 MiB stack ({error}), so the main thread runs the interpreter");
        run_interpreter();
    }
}

/// Run `run` on a new thread with a stack of `stack_size` bytes, and wait
/// for it to end. Give the error if the system refuses the thread. A panic
/// in the thread continues on this thread.
fn run_on_thread(stack_size: usize, run: fn()) -> io::Result<()> {
    let thread = std::thread::Builder::new()
        .name("interpreter".to_string())
        .stack_size(stack_size)
        .spawn(run)?;
    if let Err(panic) = thread.join() {
        // The hook showed the panic: end with the panic status
        std::panic::resume_unwind(panic);
    }
    Ok(())
}

/// Read and run lines until EXIT, QUIT or the end of input
fn run_interpreter() {
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

        if let Some(range) = list_command(input) {
            match range {
                Ok((start, end)) => list_program(&program, start, end),
                Err(e) => println!("Error: {}", e),
            }
            continue;
        }

        if input.eq_ignore_ascii_case("lvar") {
            list_variables(&executor);
            continue;
        }

        if input.eq_ignore_ascii_case("new") {
            program.clear();
            // NEW removes the variables, except A% to Z%
            executor.clear_variables();
            // The handler line belongs to the old program
            executor.clear_error_handler();
            println!("Program cleared");
            continue;
        }

        // SAVE command
        let input_upper = input.to_uppercase();
        let is_file_command = !is_lower_case_assignment(input);
        if is_file_command && input_upper.starts_with("SAVE ") {
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
        if is_file_command && input_upper.starts_with("LOAD ") {
            match extract_filename(input) {
                Ok(filename) => {
                    if let Err(e) = load_program(&mut executor, &mut program, &filename) {
                        println!("Error: {}", e);
                    }
                }
                Err(e) => println!("Error: {}", e),
            }
            continue;
        }

        // CHAIN command (LOAD and RUN)
        if is_file_command && input_upper.starts_with("CHAIN ") {
            match extract_filename(input) {
                Ok(filename) => match load_program(&mut executor, &mut program, &filename) {
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

        // RENUMBER [start[, step]]
        if let Some(arguments) = renumber_arguments(input) {
            renumber_program(&mut program, arguments);
            continue;
        }

        // AUTO [start[, step]]. Stop at end of input in AUTO mode.
        if let Some(arguments) = auto_arguments(input) {
            if !auto_mode(&stdin, &mut program, arguments) {
                println!();
                break;
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
    let tokenized = tokenize(line).map_err(|e| e.to_string())?;

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

/// Match `LIST` with an optional range (any case). Return the range, or a syntax error.
/// A bad range after `list` in lower or mixed case is a statement, such as `list = 5`.
fn list_command(input: &str) -> Option<Result<(u16, u16), String>> {
    let word = input.get(..4)?;
    if !word.eq_ignore_ascii_case("list") {
        return None;
    }
    let rest = &input[4..];
    if !rest.is_empty()
        && !rest.starts_with(|c: char| c.is_whitespace() || c.is_ascii_digit() || c == ',')
    {
        return None;
    }
    match parse_list_range(rest) {
        Err(e) => argument_error(word, e),
        range => Some(range),
    }
}

/// Give the error for a bad command argument only after the word in upper case.
/// After the word in lower or mixed case, the line is a statement, such as `list = 5`.
fn argument_error<T>(word: &str, message: impl Into<String>) -> Option<Result<T, String>> {
    (word == word.to_ascii_uppercase()).then(|| Err(message.into()))
}

/// True if the line is `<word> = ...` with the first word not in upper case.
/// `save = 5` is an assignment, but `SAVE = 5` is the command.
fn is_lower_case_assignment(input: &str) -> bool {
    input.split_once(' ').is_some_and(|(word, rest)| {
        word != word.to_ascii_uppercase() && rest.trim_start().starts_with('=')
    })
}

fn list_program(program: &ProgramStore, start: u16, end: u16) {
    if program.is_empty() {
        println!("No program");
        return;
    }

    for (line_number, line) in program.list_range(start, end) {
        match detokenize(line) {
            Ok(text) => println!("{}", text),
            Err(e) => println!("Error listing line {}: {}", line_number, e),
        }
    }
}

/// Print each variable and array with its value or its size (LVAR)
fn list_variables(executor: &Executor) {
    for line in executor.variable_listing() {
        println!("{}", line);
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
            .map_err(|e| format!("Failed to detokenize line {}: {}", line_number, e))?;
        writeln!(file, "{}", text)
            .map_err(|e| format!("Failed to write line {}: {}", line_number, e))?;
    }

    println!("Saved to {}", path);
    Ok(())
}

/// Load program from a file (.bbas added if no extension)
fn load_program(
    executor: &mut Executor,
    program: &mut ProgramStore,
    filename: &str,
) -> Result<(), String> {
    let path = resolve_program_path(filename);

    // Read file
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read file {}: {}", path, e))?;

    // Read all lines into a new store. A bad line keeps the old program.
    let loaded =
        parse_program(&content).map_err(|e| format!("{} (program in memory unchanged)", e))?;

    // Replace the program and clear the variables, as NEW does. A% to Z% stay.
    *program = loaded;
    executor.clear_variables();
    // The handler line belongs to the old program
    executor.clear_error_handler();

    println!("Loaded from {}", path);
    Ok(())
}

/// Tokenize each line of the file text into a new program store
fn parse_program(content: &str) -> Result<ProgramStore, String> {
    let mut program = ProgramStore::new();
    for (line_num, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue; // Skip empty lines
        }

        let tokenized =
            tokenize(line).map_err(|e| format!("Parse error at line {}: {}", line_num + 1, e))?;

        if let Some(line_number) = tokenized.line_number {
            // A line number with no statement deletes that line, as a typed line does.
            if tokenized.tokens.is_empty() {
                program.delete_line(line_number);
            } else {
                program.store_line(tokenized);
            }
        } else {
            return Err(format!(
                "Line {} has no line number: {}",
                line_num + 1,
                line
            ));
        }
    }
    Ok(program)
}

/// Match `DELETE start, end` (any case). Return the range, or a syntax error.
/// A bad range after `delete` in lower or mixed case is a statement, such as `delete = 5`.
fn delete_command(input: &str) -> Option<Result<(u16, u16), String>> {
    let word = input.get(..6)?;
    if !word.eq_ignore_ascii_case("delete") {
        return None;
    }
    let rest = &input[6..];
    if rest.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
        return None;
    }
    let numbers: Vec<Option<u16>> = rest.split(',').map(|n| n.trim().parse().ok()).collect();
    match numbers[..] {
        [Some(start), Some(end)] if start <= end => Some(Ok((start, end))),
        [Some(_), Some(_)] => Some(Err(
            "Syntax error: DELETE start is more than end".to_string()
        )),
        _ => argument_error(word, "Syntax error: use DELETE start, end"),
    }
}

/// Match `RENUMBER`, `RENUMBER start` or `RENUMBER start, step` (any case).
/// Return the start and the step (default 10 and 10), or an error for bad arguments.
/// Bad arguments after `renumber` in lower or mixed case are a statement, such as `renumber = 6`.
fn renumber_arguments(input: &str) -> Option<Result<(u16, u16), String>> {
    let word = input.get(..8)?;
    if !word.eq_ignore_ascii_case("renumber") {
        return None;
    }
    let rest = &input[8..];
    if !rest.is_empty() && !rest.starts_with(|c: char| c.is_whitespace() || c.is_ascii_digit()) {
        return None;
    }
    let parts: Vec<&str> = rest.split(',').map(str::trim).collect();
    if parts.len() > 2 || (parts.len() == 2 && parts.contains(&"")) {
        return argument_error(word, "Syntax error");
    }
    let mut numbers = [10, 10];
    for (number, part) in numbers.iter_mut().zip(&parts) {
        if !part.is_empty() {
            match part.parse() {
                Ok(value) => *number = value,
                Err(_) => return argument_error(word, "Syntax error"),
            }
        }
    }
    Some(Ok((numbers[0], numbers[1])))
}

/// Renumber the program. Print `Failed at <line>` for each line with a jump
/// to a line that does not exist.
fn renumber_program(program: &mut ProgramStore, arguments: Result<(u16, u16), String>) {
    match arguments.and_then(|(start, step)| program.renumber(start, step)) {
        Ok(failed) => {
            for line in failed {
                println!("Failed at {}", line);
            }
        }
        Err(e) => println!("Error: {}", e),
    }
}

/// Match `AUTO`, `AUTO start` or `AUTO start, step` (any case).
/// Return the start and the step (default 10 and 10), or an error for bad arguments.
/// Bad arguments after `auto` in lower or mixed case are a statement, such as `auto = 7`.
fn auto_arguments(input: &str) -> Option<Result<(u32, u32), String>> {
    let word = input.get(..4)?;
    if !word.eq_ignore_ascii_case("auto") {
        return None;
    }
    let rest = &input[4..];
    if !rest.is_empty() && !rest.starts_with(|c: char| c.is_whitespace() || c.is_ascii_digit()) {
        return None;
    }
    let parts: Vec<&str> = rest.split(',').map(str::trim).collect();
    if parts.len() > 2 || (parts.len() == 2 && parts.contains(&"")) {
        return argument_error(word, "Syntax error");
    }
    let mut numbers = [10, 10];
    for (number, part) in numbers.iter_mut().zip(&parts) {
        if !part.is_empty() {
            match part.parse() {
                Ok(value) => *number = value,
                Err(_) => return argument_error(word, "Syntax error"),
            }
        }
    }
    match numbers {
        [_, 0] => Some(Err("Silly".to_string())),
        [start, step] => Some(Ok((start, step))),
    }
}

/// Prompt with the next line number and store each typed line with that number.
/// End the mode for an empty line or a number above 65535.
/// Return false if the input ended.
fn auto_mode(
    stdin: &io::Stdin,
    program: &mut ProgramStore,
    arguments: Result<(u32, u32), String>,
) -> bool {
    let (mut number, step) = match arguments {
        Ok(arguments) => arguments,
        Err(e) => {
            println!("Error: {}", e);
            return true;
        }
    };
    let mut line_buffer = String::new();
    while number <= u32::from(u16::MAX) {
        print!("{} ", number);
        io::stdout().flush().unwrap();
        line_buffer.clear();
        if matches!(stdin.read_line(&mut line_buffer), Ok(0) | Err(_)) {
            return false;
        }
        let text = line_buffer.trim();
        if text.is_empty() {
            return true;
        }
        match tokenize(&format!("{} {}", number, text)) {
            Ok(tokenized) => {
                program.store_line(tokenized);
                number = number.saturating_add(step);
            }
            Err(e) => println!("Error: {}", e),
        }
    }
    println!("Error: Too big");
    true
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
    println!("  LIST [start][,[end]]     - List the program, or a range of lines");
    println!("  RUN                      - Run the stored program");
    println!("  NEW                      - Clear the program");
    println!("  LVAR                     - List the variables and arrays");
    println!("  CLEAR                    - Remove all variables and arrays");
    println!("  DELETE 20, 40            - Delete lines 20 to 40");
    println!("  RENUMBER [start[,step]]  - Renumber the lines (default 10,10)");
    println!("  AUTO [start[,step]]      - Number each typed line (empty line ends)");
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
    println!("  TRACE ON / OFF / n       - Show each line number (below n) as it runs");
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
    use super::{is_closed_output_panic, run_on_thread};
    use std::sync::atomic::{AtomicBool, Ordering};

    static THREAD_RAN: AtomicBool = AtomicBool::new(false);

    fn mark_thread_ran() {
        THREAD_RAN.store(true, Ordering::SeqCst);
    }

    #[test]
    fn test_run_on_thread_runs_the_function() {
        assert!(run_on_thread(1024 * 1024, mark_thread_ran).is_ok());
        assert!(THREAD_RAN.load(Ordering::SeqCst));
    }

    #[test]
    fn test_run_on_thread_gives_an_error_when_the_system_refuses_the_stack() {
        fn not_called() {
            panic!("the thread must not start");
        }
        assert!(run_on_thread(1 << 50, not_called).is_err());
    }

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
