//! Issue #80: OR and EOR give the BBC BASIC results.
//!
//! The parser mapped the keyword code of EOR to OR and did not know the code
//! of OR. OR ended the expression, and EOR gave the result of OR.

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

/// Run the program in a separate thread with a time limit of 5 seconds, so that
/// a loop that does not end fails the test and does not stop the test run.
fn run_with_limit(source: &[&str]) -> (Result<(), String>, String) {
    let source: Vec<String> = source.iter().map(|line| line.to_string()).collect();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let lines: Vec<&str> = source.iter().map(String::as_str).collect();
        let _ = sender.send(run(&lines));
    });
    receiver
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("program ends within 5 seconds and does not panic")
}

#[test]
fn test_zero_or_five_is_five() {
    let (result, output) = run_with_limit(&["10 PRINT 0 OR 5"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_six_eor_three_is_five() {
    let (result, output) = run_with_limit(&["10 PRINT 6 EOR 3"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_six_and_three_is_two() {
    let (result, output) = run_with_limit(&["10 PRINT 6 AND 3"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "2");
}

#[test]
fn test_and_binds_more_than_or() {
    // 1 OR (2 AND 4) is 1. (1 OR 2) AND 4 is 0.
    let (result, output) = run_with_limit(&["10 PRINT 1 OR 2 AND 4"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_if_zero_or_one_takes_then_branch() {
    let (result, output) = run_with_limit(&["10 IF 0 OR 1 THEN PRINT \"Y\" ELSE PRINT \"N\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "Y\n");
}

#[test]
fn test_until_with_or_false_ends_at_three() {
    let (result, output) = run_with_limit(&[
        "10 A% = 0",
        "20 REPEAT",
        "30 A% = A% + 1",
        "40 UNTIL A% = 3 OR FALSE",
        "50 PRINT A%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "3");
}

#[test]
fn test_two_string_comparisons_joined_by_or() {
    let (result, output) = run_with_limit(&[
        "10 A$ = \"Y\"",
        "20 IF A$ = \"X\" OR A$ = \"Y\" THEN PRINT \"MATCH\" ELSE PRINT \"NO\"",
        "30 A$ = \"Z\"",
        "40 IF A$ = \"X\" OR A$ = \"Y\" THEN PRINT \"MATCH\" ELSE PRINT \"NO\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "MATCH\nNO\n");
}

#[test]
fn test_not_zero_or_zero_is_minus_one() {
    let (result, output) = run_with_limit(&["10 PRINT NOT 0 OR 0"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "-1");
}
