//! Tests that the resident integer variables A% to Z% always exist (issue #89).

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

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

/// Run the binary with the given input lines and return its stdout.
/// Kill the binary and fail if it runs for more than 5 seconds.
fn run_binary(input: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_bbc-basic-interpreter"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("binary starts");
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(input.as_bytes())
        .expect("input is written");

    let mut stdout = child.stdout.take().expect("stdout is piped");
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        stdout
            .read_to_string(&mut text)
            .expect("stdout is readable");
        text
    });

    let deadline = Instant::now() + Duration::from_secs(5);
    while child.try_wait().expect("child status").is_none() {
        if Instant::now() > deadline {
            child.kill().expect("child is killed");
            panic!("the binary ran for more than 5 seconds");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    reader.join().expect("reader thread ends")
}

#[test]
fn test_resident_integer_that_was_never_set_is_zero() {
    let (result, output) = run(&["10 PRINT I%"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0\n");
}

#[test]
fn test_resident_integer_can_be_incremented_before_it_is_set() {
    let (result, output) = run(&["10 I% = I% + 1", "20 PRINT I%"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n");
}

#[test]
fn test_clear_keeps_resident_integer() {
    let (result, output) = run(&["10 A% = 5", "20 CLEAR", "30 PRINT A%"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "5\n");
}

#[test]
fn test_clear_removes_real_variable() {
    let (result, _) = run(&["10 B = 1", "20 CLEAR", "30 PRINT B"]);

    let error = result.expect_err("a read of B after CLEAR fails");
    assert!(error.contains("line 30"), "error: {error}");
}

#[test]
fn test_two_letter_integer_that_was_never_set_is_error() {
    let (result, _) = run(&["10 PRINT AB%"]);

    let error = result.expect_err("AB% is not a resident variable");
    assert!(error.contains("line 10"), "error: {error}");
}

#[test]
fn test_lower_case_integer_that_was_never_set_is_error() {
    let (result, _) = run(&["10 PRINT a%"]);

    let error = result.expect_err("a% is not a resident variable");
    assert!(error.contains("line 10"), "error: {error}");
}

#[test]
fn test_real_and_string_of_resident_letter_are_errors() {
    let (real_result, _) = run(&["10 PRINT A"]);
    let (string_result, _) = run(&["10 PRINT A$"]);

    assert!(real_result.is_err(), "A is not set: {real_result:?}");
    assert!(string_result.is_err(), "A$ is not set: {string_result:?}");
}

#[test]
fn test_local_resident_integer_is_zero_and_restored() {
    let (result, output) = run(&[
        "10 A% = 7",
        "20 PROCT",
        "30 PRINT A%",
        "40 END",
        "100 DEF PROCT",
        "110 LOCAL A%",
        "120 PRINT A%",
        "130 A% = 9",
        "140 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0\n7\n");
}

#[test]
fn test_local_resident_integer_that_was_never_set_is_restored_to_zero() {
    let (result, output) = run(&[
        "10 PROCT",
        "20 PRINT A%",
        "30 END",
        "100 DEF PROCT",
        "110 LOCAL A%",
        "120 A% = 9",
        "130 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0\n");
}

#[test]
fn test_lvar_in_new_session_shows_no_variables() {
    let output = run_binary("LVAR\nPRINT \"END\"\nEXIT\n");

    assert!(!output.contains('%'), "output: {output}");
    assert!(!output.contains(" = "), "output: {output}");
}

#[test]
fn test_lvar_shows_resident_integer_that_is_not_zero() {
    let output = run_binary("Z% = 3\nLVAR\nEXIT\n");

    assert!(output.contains("Z% = 3\n"), "output: {output}");
    assert!(!output.contains("A% ="), "output: {output}");
}

#[test]
fn test_typed_print_of_resident_integer_that_was_never_set_is_zero() {
    let output = run_binary("PRINT I%\nEXIT\n");

    assert!(output.contains("0\n"), "output: {output}");
    assert!(!output.contains("Error"), "output: {output}");
}
