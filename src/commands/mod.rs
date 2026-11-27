//! The typed commands and statements of the prompt: `RUN`, `LIST`, `NEW`,
//! `SAVE` and the others, a program line, or a statement to run now.
//!
//! The command prompt of the binary and the run pane of the IDE both call
//! `run_command` (issue #171). The text goes through `Executor::write_text`,
//! and AUTO reads its lines with `Executor::read_text_line`. With no console,
//! these are stdout and stdin, as before.

use crate::executor::Executor;
use crate::program::{parse_list_range, ProgramStore};
use crate::runner::{run_program_with_outcome, run_typed_line, RunOutcome};
use crate::tokenizer::{detokenize, expand_short_form, tokenize};

/// Write one line of text through the executor, as `println!` does.
macro_rules! say {
    ($executor:expr) => {
        $executor.write_text("\n")
    };
    ($executor:expr, $($arg:tt)*) => {
        $executor.write_text(&format!("{}\n", format_args!($($arg)*)))
    };
}

/// What the front end does after a command with no error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandOutcome {
    /// Show the prompt again
    Done,
    /// EXIT, QUIT, or a program that ran QUIT: leave the interpreter
    Quit,
    /// The input ended in AUTO mode
    InputEnded,
}

/// Run one typed line, with no spaces at its start and end. An error gives
/// its text, and the front end shows it as `Error: <text>`.
pub fn run_command(
    executor: &mut Executor,
    program: &mut ProgramStore,
    input: &str,
) -> Result<CommandOutcome, String> {
    use CommandOutcome::{Done, InputEnded, Quit};

    // A short form at the start, such as L., is the full keyword (issue 167)
    let expanded = expand_short_form(input);
    let input = expanded.as_str();

    // Check for commands
    if input.eq_ignore_ascii_case("exit") || input.eq_ignore_ascii_case("quit") {
        return Ok(Quit);
    }

    if input.eq_ignore_ascii_case("help") {
        print_help(executor);
        return Ok(Done);
    }

    if input.is_empty() {
        return Ok(Done);
    }

    // Handle special commands
    if input.eq_ignore_ascii_case("run") {
        return run_outcome(run_program_with_outcome(executor, program));
    }

    if let Some(range) = list_command(input) {
        let (start, end) = range?;
        list_program(executor, program, start, end);
        return Ok(Done);
    }

    if input.eq_ignore_ascii_case("lvar") {
        list_variables(executor);
        return Ok(Done);
    }

    if input.eq_ignore_ascii_case("new") {
        program.clear();
        // NEW removes the variables, except A% to Z%
        executor.clear_variables();
        // The handler line belongs to the old program
        executor.clear_error_handler();
        // NEW removes the libraries of INSTALL and each routine (issue #185)
        executor.clear_libraries_and_routines();
        say!(executor, "Program cleared");
        return Ok(Done);
    }

    // SAVE command
    let input_upper = input.to_uppercase();
    let is_file_command = !is_lower_case_assignment(input);
    if is_file_command && input_upper.starts_with("SAVE ") {
        let filename = extract_filename(input)?;
        save_program(executor, program, &filename)?;
        return Ok(Done);
    }

    // LOAD command
    if is_file_command && input_upper.starts_with("LOAD ") {
        let filename = extract_filename(input)?;
        load_program(executor, program, &filename)?;
        // LOAD removes the libraries of INSTALL and each routine (issue
        // #185). CHAIN keeps the libraries, as the manual says.
        executor.clear_libraries_and_routines();
        return Ok(Done);
    }

    // CHAIN command (LOAD and RUN)
    if is_file_command && input_upper.starts_with("CHAIN ") {
        let filename = extract_filename(input)?;
        load_program(executor, program, &filename)?;
        return run_outcome(run_program_with_outcome(executor, program));
    }

    // RENUMBER [start[, step]]
    if let Some(arguments) = renumber_arguments(input) {
        renumber_program(executor, program, arguments)?;
        return Ok(Done);
    }

    // AUTO [start[, step]]. Stop at end of input in AUTO mode.
    if let Some(arguments) = auto_arguments(input) {
        return Ok(if auto_mode(executor, program, arguments?)? {
            Done
        } else {
            InputEnded
        });
    }

    // *CAT command (catalog files)
    if let Some(directory) = cat_directory(input) {
        catalog_files(executor, directory)?;
        return Ok(Done);
    }

    if let Some(range) = delete_command(input) {
        let (start, end) = range?;
        program.delete_range(start, end);
        return Ok(Done);
    }

    // Process the line (either store or execute)
    run_outcome(process_line(executor, program, input))
}

/// The command outcome of a run.
fn run_outcome(outcome: Result<RunOutcome, String>) -> Result<CommandOutcome, String> {
    match outcome? {
        RunOutcome::Ended => Ok(CommandOutcome::Done),
        RunOutcome::Quit => Ok(CommandOutcome::Quit),
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

fn list_program(executor: &mut Executor, program: &ProgramStore, start: u16, end: u16) {
    if program.is_empty() {
        say!(executor, "No program");
        return;
    }

    for (line_number, line) in program.list_range(start, end) {
        match detokenize(line) {
            Ok(text) => say!(executor, "{}", text),
            Err(e) => say!(executor, "Error listing line {}: {}", line_number, e),
        }
    }
}

/// Print each variable and array with its value or its size (LVAR)
fn list_variables(executor: &mut Executor) {
    for line in executor.variable_listing() {
        say!(executor, "{}", line);
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
pub(crate) fn resolve_program_path(filename: &str) -> String {
    if std::path::Path::new(filename).extension().is_some() {
        filename.to_string()
    } else {
        format!("{}.bbas", filename)
    }
}

/// Save current program to a file (.bbas added if no extension)
fn save_program(
    executor: &mut Executor,
    program: &mut ProgramStore,
    filename: &str,
) -> Result<(), String> {
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

    // INSTALL searches the directory of this file first (issue #186)
    program.set_file(Some(path.clone()));
    say!(executor, "Saved to {}", path);
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
    // INSTALL searches the directory of this file first (issue #186)
    program.set_file(Some(path.clone()));
    executor.clear_variables();
    // The handler line belongs to the old program
    executor.clear_error_handler();

    say!(executor, "Loaded from {}", path);
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
fn renumber_program(
    executor: &mut Executor,
    program: &mut ProgramStore,
    arguments: Result<(u16, u16), String>,
) -> Result<(), String> {
    let failed = arguments.and_then(|(start, step)| program.renumber(start, step))?;
    for line in failed {
        say!(executor, "Failed at {}", line);
    }
    Ok(())
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
/// End the mode for an empty line or a number above 65535 (the error Too big).
/// Return false if the input ended.
fn auto_mode(
    executor: &mut Executor,
    program: &mut ProgramStore,
    (mut number, step): (u32, u32),
) -> Result<bool, String> {
    while number <= u32::from(u16::MAX) {
        executor.write_text(&format!("{} ", number));
        let Some(line) = executor.read_text_line() else {
            return Ok(false);
        };
        let text = line.trim();
        if text.is_empty() {
            return Ok(true);
        }
        match tokenize(&format!("{} {}", number, text)) {
            Ok(tokenized) => {
                program.store_line(tokenized);
                number = number.saturating_add(step);
            }
            Err(e) => say!(executor, "Error: {}", e),
        }
    }
    Err("Too big".to_string())
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
fn catalog_files(executor: &mut Executor, directory: &str) -> Result<(), String> {
    let path = if directory.is_empty() { "." } else { directory };
    if !std::path::Path::new(path).is_dir() {
        return Err(format!("Directory not found: {}", path));
    }
    let paths = std::fs::read_dir(path).map_err(|e| format!("Failed to read directory: {}", e))?;

    if directory.is_empty() {
        say!(executor, "\nCatalog:");
    } else {
        say!(executor, "\nCatalog: {}", directory);
    }
    say!(executor, "{:<30} {:>10}  Modified", "Filename", "Size");
    say!(executor, "{}", "-".repeat(60));

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

            say!(executor, "{:<30} {:>10}  {}", filename_str, size, modified);
            count += 1;
        }
    }

    if count == 0 {
        say!(executor, "(no .bbas or .bas files found)");
    } else {
        say!(executor, "\n{} file(s)", count);
    }

    Ok(())
}

fn print_help(executor: &mut Executor) {
    let text = r#"BBC BASIC Interpreter - Available Commands:

Program Mode (with line numbers):
  10 PRINT "HELLO"        - Store program line
  20 GOTO 10               - Store line with GOTO
  10                       - Delete line 10

Immediate Commands:
  LIST [start][,[end]]     - List the program, or a range of lines
  RUN                      - Run the stored program
  NEW                      - Clear the program
  LVAR                     - List the variables and arrays
  CLEAR                    - Remove all variables and arrays
  DELETE 20, 40            - Delete lines 20 to 40
  RENUMBER [start[,step]]  - Renumber the lines (default 10,10)
  AUTO [start[,step]]      - Number each typed line (empty line ends)
  SAVE "filename"          - Save program to filename.bbas
  LOAD "filename"          - Load program from filename.bbas
  CHAIN "filename"         - Load and run program
  *CAT                     - List all .bbas and .bas files
  *CAT directory           - List the .bbas and .bas files in directory

Immediate Mode (no line numbers):
  A% = 42                  - Execute immediately
  PRINT "text"             - Execute immediately

Statements:
  LET A% = 42              - Assign integer variable
  B = 3.14                 - Assign real variable (LET optional)
  C$ = "HELLO"             - Assign string variable
  PRINT "text", A%, B      - Print values
  FOR I% = 1 TO 10         - Start FOR loop
  NEXT I%                  - End FOR loop
  INPUT A%, B$             - Input variables
  DIM A%(10)               - Dimension array
  GOTO 100                 - Jump to line
  GOSUB 1000               - Call subroutine
  RETURN                   - Return from subroutine
  DEF PROC name(params)    - Define procedure
  PROC name(args)          - Call procedure
  ENDPROC                  - End procedure
  REM comment              - Comment
  TRACE ON / OFF / n       - Show each line number (below n) as it runs
  END                      - End program

Examples:
  10 PRINT "Hello"
  20 GOTO 10
  LIST
  RUN
  SAVE "myprog"
  LOAD "myprog"

Variable Types:
  A%  - Integer variable
  B   - Real (float) variable
  C$  - String variable

"#;
    executor.write_text(text);
}
