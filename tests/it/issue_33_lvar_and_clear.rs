//! Tests that LVAR lists the variables and CLEAR removes them (issue #33).

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
fn test_clear_in_program_removes_variable() {
    // AB% is not a resident variable (A% to Z% stay after CLEAR, issue #89)
    let (result, output) = run(&["10 AB% = 5", "20 PRINT AB%", "30 CLEAR", "40 PRINT AB%"]);

    assert_eq!(output, "5\n");
    let error = result.expect_err("a read after CLEAR fails");
    assert!(is_no_such_variable(&error, "AB%"), "error: {error}");
    assert!(error.contains("line 40"), "error: {error}");
}

/// True if the error is the no such variable error for `name`.
fn is_no_such_variable(error: &str, name: &str) -> bool {
    error.contains(&format!("No such variable: {name} at line "))
}

#[test]
fn test_clear_removes_array() {
    let (result, _) = run(&["10 DIM B(3)", "20 CLEAR", "30 PRINT B(1)"]);

    let error = result.expect_err("an array read after CLEAR fails");
    assert!(is_no_such_variable(&error, "B"), "error: {error}");
}

#[test]
fn test_clear_keeps_procedures() {
    let (result, output) = run(&[
        "10 CLEAR",
        "20 PROCHI",
        "30 END",
        "40 DEF PROCHI",
        "50 PRINT \"HI\"",
        "60 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "HI\n");
}

#[test]
fn test_clear_inside_for_loop_closes_loop() {
    let (result, output) = run(&[
        "10 FOR I% = 1 TO 3",
        "20 PRINT \"X\"",
        "30 CLEAR",
        "40 NEXT",
    ]);

    assert_eq!(output, "X\n");
    let error = result.expect_err("NEXT after CLEAR has no open loop");
    assert!(error.contains("line 40"), "error: {error}");
}

#[test]
fn test_clear_inside_procedure_closes_scope() {
    let (result, output) = run(&[
        "10 PROCA",
        "20 PRINT \"BACK\"",
        "30 END",
        "40 DEF PROCA",
        "50 CLEAR",
        "60 ENDPROC",
    ]);

    assert_eq!(output, "");
    let error = result.expect_err("ENDPROC after CLEAR has no open scope");
    assert_eq!(error, "No PROC at line 60");
}

#[test]
fn test_clear_resets_data_pointer() {
    let (result, output) = run(&[
        "10 READ A%",
        "20 CLEAR",
        "30 READ B%",
        "40 PRINT B%",
        "50 DATA 7, 8",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "7\n");
}

#[test]
fn test_lvar_lists_variables_and_array_in_name_order() {
    let output = run_binary("Z = 1.5\nA% = 3\nM$ = \"HI\"\nDIM B%(4, 2)\nLVAR\nEXIT\n");

    assert!(
        output.contains("A% = 3\nM$ = \"HI\"\nZ = 1.5\nB%(4,2)\n"),
        "output: {output}"
    );
}

#[test]
fn test_typed_clear_then_lvar_shows_nothing() {
    // AB% is not a resident variable (A% to Z% stay after CLEAR, issue #89)
    let output = run_binary("AB% = 3\nDIM B(2)\nCLEAR\nLVAR\nEXIT\n");

    assert!(!output.contains("AB% ="), "output: {output}");
    assert!(!output.contains("B("), "output: {output}");
    assert!(!output.contains("Error"), "output: {output}");
}

#[test]
fn test_lvar_in_program_line_is_parse_error() {
    let (result, _) = run(&["10 LVAR"]);

    let error = result.expect_err("LVAR is not a program statement");
    assert_eq!(error, "Syntax error at line 10: Unknown statement: LVAR");
}

#[test]
fn test_help_lists_lvar_and_clear() {
    let output = run_binary("HELP\nEXIT\n");

    assert!(output.contains("LVAR"), "output: {output}");
    assert!(output.contains("CLEAR"), "output: {output}");
}
