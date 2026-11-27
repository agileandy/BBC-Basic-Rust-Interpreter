//! Issue 62: a character that the interpreter does not know gives an error.

use bbc_basic_interpreter::error::BBCBasicError;
use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;

/// Store the source lines, run the program and return the result and the output.
fn run(source: &[&str]) -> (Result<(), String>, String) {
    let mut executor = Executor::new();
    let mut program = ProgramStore::new();
    for line in source {
        program.store_line(tokenize(line).expect("line tokenizes"));
    }
    let result = run_program(&mut executor, &mut program);
    (result, executor.get_output().to_string())
}

/// Return the message of the syntax error that the line gives.
fn syntax_error(line: &str) -> String {
    match tokenize(line) {
        Err(BBCBasicError::SyntaxError { message, .. }) => message,
        other => panic!("{:?} gave {:?}", line, other),
    }
}

#[test]
fn test_print_exclamation_mark_gives_unknown_character_error() {
    // Issue #63 makes `~` a known character, so this test uses `!`
    assert_eq!(syntax_error("10 PRINT !5"), "Unknown character '!'");
}

#[test]
fn test_question_mark_gives_unknown_character_error() {
    assert_eq!(syntax_error("10 X = ?100"), "Unknown character '?'");
}

#[test]
fn test_program_with_unknown_character_line_does_not_start() {
    // The line with the error is not stored, so the program has no lines.
    let mut executor = Executor::new();
    let mut program = ProgramStore::new();
    for line in ["10 PRINT !5", "20 X = ?100"] {
        if let Ok(tokenized) = tokenize(line) {
            program.store_line(tokenized);
        }
    }

    let result = run_program(&mut executor, &mut program);

    assert_eq!(result, Err("No program to run".to_string()));
    assert_eq!(executor.get_output(), "");
}

#[test]
fn test_string_rem_and_data_with_unknown_characters_still_run() {
    let (result, output) = run(&[
        "10 PRINT \"~?!#@|\"",
        "20 REM ~?!@|\\[]{}`",
        "30 READ A$",
        "40 PRINT A$",
        "50 DATA \"[~]\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "~?!#@|\n[~]\n");
}

#[test]
fn test_file_statements_with_hash_still_work() {
    let path = std::env::temp_dir().join(format!("issue_62_{}.txt", std::process::id()));
    let path = path.to_string_lossy().to_string();
    let open_out = format!("10 F% = OPENOUT(\"{}\")", path);
    let open_in = format!("40 G% = OPENIN(\"{}\")", path);

    let (result, output) = run(&[
        &open_out,
        "20 PRINT#F%, 42",
        "30 CLOSE#F%",
        &open_in,
        "50 INPUT#G%, A",
        "60 CLOSE#G%",
        "70 PRINT A",
    ]);
    let _ = std::fs::remove_file(&path);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "42");
}

#[test]
fn test_print_number_that_starts_with_decimal_point() {
    let (result, output) = run(&["10 PRINT .5"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "0.5");
}

#[test]
fn test_compare_number_that_starts_with_decimal_point() {
    let (result, output) = run(&["10 IF .5 < 1 THEN PRINT \"OK\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "OK\n");
}
