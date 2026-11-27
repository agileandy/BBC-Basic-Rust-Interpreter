//! Tests that RUN, NEW, LOAD and CHAIN remove the variables, except the
//! resident integer variables A% to Z% (issue #92).

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::{run_program, run_typed_line};
use bbc_basic_interpreter::tokenizer::tokenize;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Run the binary in the package directory with the given input lines and
/// return its stdout. Kill the binary and fail if it runs for more than
/// 5 seconds.
fn run_binary(input: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_bbc-basic-interpreter"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
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

/// Assert that the binary printed an error and did not print the value 2.
fn assert_x_does_not_exist(output: &str) {
    assert!(output.contains("Error:"), "output was: {}", output);
    assert!(!output.contains(">          2\n"), "output was: {}", output);
}

/// Store the source lines in a new program.
fn store(source: &[&str]) -> ProgramStore {
    let mut program = ProgramStore::new();
    for line in source {
        program.store_line(tokenize(line).expect("line tokenizes"));
    }
    program
}

/// Run a typed line with no line number.
fn type_line(executor: &mut Executor, program: &ProgramStore, line: &str) {
    let tokenized = tokenize(line).expect("line tokenizes");
    run_typed_line(executor, program, &tokenized).expect("typed line runs");
}

#[test]
fn test_run_removes_variable_set_by_typed_line() {
    let mut executor = Executor::new();
    let mut program = store(&["10 PRINT X"]);
    type_line(&mut executor, &program, "X = 2");

    let result = run_program(&mut executor, &mut program);

    let error = result.expect_err("X does not exist after RUN");
    assert!(error.contains("at line 10"), "error was: {}", error);
    assert_eq!(executor.get_output(), "");
}

#[test]
fn test_run_keeps_resident_integer_set_by_typed_line() {
    let mut executor = Executor::new();
    let mut program = store(&["10 PRINT A%"]);
    type_line(&mut executor, &program, "A% = 2");

    let result = run_program(&mut executor, &mut program);

    assert_eq!(result, Ok(()));
    assert_eq!(executor.get_output(), "         2\n");
}

#[test]
fn test_program_that_runs_two_times_gives_same_output() {
    let mut executor = Executor::new();
    // Line 30 reads X before the program sets it. The error goes to line
    // 100, which sets X and the array. If X stays after the first run,
    // the second run prints X at line 30 and does not go to line 100.
    let mut program = store(&[
        "10 ON ERROR GOTO 100",
        "20 DIM N(2)",
        "30 PRINT X",
        "40 END",
        "100 X = 5",
        "110 N(1) = N(1) + X",
        "120 PRINT \"SET \"; N(1)",
    ]);

    assert_eq!(run_program(&mut executor, &mut program), Ok(()));
    let first = executor.get_output().to_string();
    assert_eq!(run_program(&mut executor, &mut program), Ok(()));
    let both = executor.get_output().to_string();

    assert_eq!(first, "SET 5\n");
    assert_eq!(both, "SET 5\nSET 5\n");
}

#[test]
fn test_new_removes_variables() {
    let output = run_binary("X = 2\nNEW\nPRINT X\nEXIT\n");

    assert_x_does_not_exist(&output);
}

#[test]
fn test_load_removes_variables() {
    let output = run_binary("X = 2\nLOAD \"examples/test_if.bas\"\nPRINT X\nEXIT\n");

    assert_x_does_not_exist(&output);
}

#[test]
fn test_new_keeps_resident_integer() {
    let output = run_binary("A% = 2\nNEW\nPRINT A%\nEXIT\n");

    assert!(output.contains(">          2\n"), "output was: {}", output);
}

#[test]
fn test_typed_line_keeps_variables() {
    let output = run_binary("X = 2\nPRINT X\nEXIT\n");

    assert!(output.contains(">          2\n"), "output was: {}", output);
}
