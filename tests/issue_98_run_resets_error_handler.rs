//! Tests that RUN, NEW and LOAD remove the ON ERROR handler, and that CLEAR
//! keeps it (issue #98).

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::{run_program, run_typed_line, RunOutcome};
use bbc_basic_interpreter::tokenizer::tokenize;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Store the source lines in a new program.
fn store(source: &[&str]) -> ProgramStore {
    let mut program = ProgramStore::new();
    for line in source {
        program.store_line(tokenize(line).expect("line tokenizes"));
    }
    program
}

/// Run a typed line with no line number and return the result.
fn type_line(
    executor: &mut Executor,
    program: &ProgramStore,
    line: &str,
) -> Result<RunOutcome, String> {
    let tokenized = tokenize(line).expect("line tokenizes");
    run_typed_line(executor, program, &tokenized)
}

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

#[test]
fn test_run_removes_handler_of_earlier_run() {
    let mut executor = Executor::new();
    let mut first = store(&["10 ON ERROR GOTO 100", "20 END", "100 PRINT \"H\""]);
    run_program(&mut executor, &mut first).expect("first run ends");
    let mut second = store(&["10 PRINT 1/0", "100 PRINT \"H\""]);

    let result = run_program(&mut executor, &mut second);

    assert_eq!(result, Err("Division by zero at line 10".to_string()));
    assert_eq!(executor.get_output(), "");
}

#[test]
fn test_clear_keeps_handler() {
    let mut executor = Executor::new();
    let mut program = store(&[
        "10 ON ERROR GOTO 100",
        "20 CLEAR",
        "30 PRINT 1/0",
        "40 END",
        "100 PRINT \"H\"",
    ]);

    let result = run_program(&mut executor, &mut program);

    assert_eq!(result, Ok(()));
    assert_eq!(executor.get_output(), "H\n");
}

#[test]
fn test_typed_handler_applies_to_typed_lines_until_run() {
    let mut executor = Executor::new();
    let mut program = store(&["10 PRINT 1/0", "20 END", "100 PRINT \"H\""]);
    type_line(&mut executor, &program, "ON ERROR GOTO 100").expect("typed line runs");

    let typed = type_line(&mut executor, &program, "GOTO 10");
    let run = run_program(&mut executor, &mut program);

    assert_eq!(typed, Ok(RunOutcome::Ended));
    assert_eq!(run, Err("Division by zero at line 10".to_string()));
    assert_eq!(executor.get_output(), "H\n");
}

#[test]
fn test_new_removes_handler() {
    let output =
        run_binary("ON ERROR GOTO 100\nNEW\n10 PRINT 1/0\n100 PRINT \"H\"\nGOTO 10\nEXIT\n");

    assert!(
        output.contains("Division by zero at line 10"),
        "output was: {}",
        output
    );
    assert!(!output.contains("> H\n"), "output was: {}", output);
}

#[test]
fn test_load_removes_handler() {
    let output = run_binary(
        "ON ERROR GOTO 100\nLOAD \"examples/test_if.bas\"\n10 PRINT 1/0\n100 PRINT \"H\"\nGOTO 10\nEXIT\n",
    );

    assert!(
        output.contains("Division by zero at line 10"),
        "output was: {}",
        output
    );
    assert!(!output.contains("> H\n"), "output was: {}", output);
}
