//! Issue 59: text after a line number in a THEN or an ELSE branch is an error.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;

/// Store the source lines, run the program and return the result and the output.
///
/// The program runs in a thread with a time limit, so a jump defect that
/// loops without end fails the test and does not hang the test run.
fn run(source: &[&str]) -> (Result<(), String>, String) {
    let lines: Vec<String> = source.iter().map(|line| line.to_string()).collect();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut executor = Executor::new();
        let mut program = ProgramStore::new();
        for line in &lines {
            program.store_line(tokenize(line).expect("line tokenizes"));
        }
        let result = run_program(&mut executor, &mut program);
        let _ = sender.send((result, executor.get_output().to_string()));
    });
    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("program stops in 5 seconds")
}

#[test]
fn test_if_then_line_number_with_calculation_does_not_start() {
    let (result, output) = run(&[
        "10 PRINT \"START\"",
        "20 IF 1 THEN 30+1",
        "30 PRINT \"THIRTY\"",
    ]);

    let error = result.expect_err("program does not start");
    assert!(
        error.contains("Expected end of statement after line number"),
        "error was {error}"
    );
    assert!(error.contains("line 20"), "error was {error}");
    assert_eq!(output, "");
}

#[test]
fn test_if_else_line_number_with_calculation_does_not_start() {
    let (result, output) = run(&[
        "10 PRINT \"START\"",
        "20 IF 0 THEN 30 ELSE 30+1",
        "30 PRINT \"THIRTY\"",
    ]);

    let error = result.expect_err("program does not start");
    assert!(
        error.contains("Expected end of statement after line number"),
        "error was {error}"
    );
    assert_eq!(output, "");
}

#[test]
fn test_if_then_line_number_colon_print_still_jumps() {
    let (result, output) = run(&[
        "10 IF 1 THEN 30 : PRINT 1",
        "20 PRINT \"SKIPPED\"",
        "30 PRINT \"THIRTY\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "THIRTY\n");
}
