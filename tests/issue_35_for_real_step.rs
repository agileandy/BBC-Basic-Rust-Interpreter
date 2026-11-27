//! Issue 35: a FOR loop with a real loop variable keeps the fraction parts
//! of the start, end and step values.
//!
//! A loop variable with a `%` name is an integer loop. Its start, end and
//! step values are truncated to integers.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;

/// Store the source lines, run the program and return the result and the output.
///
/// The run has a time limit, so that a loop defect fails the test and does
/// not stop the test run.
fn run(source: &[&str]) -> (Result<(), String>, String) {
    let source: Vec<String> = source.iter().map(|line| line.to_string()).collect();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut executor = Executor::new();
        let mut program = ProgramStore::new();
        for line in &source {
            program.store_line(tokenize(line).expect("line tokenizes"));
        }
        let result = run_program(&mut executor, &mut program);
        let _ = sender.send((result, executor.get_output().to_string()));
    });
    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("the program stops in 5 seconds")
}

#[test]
fn test_real_loop_with_half_step() {
    let (result, output) = run(&["10 FOR I = 1 TO 2 STEP 0.5", "20 PRINT I", "30 NEXT I"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n1.5\n2\n");
}

#[test]
fn test_real_loop_with_negative_quarter_step() {
    let (result, output) = run(&["10 FOR I = 2 TO 1 STEP -0.25", "20 PRINT I", "30 NEXT I"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "2\n1.75\n1.5\n1.25\n1\n");
}

#[test]
fn test_real_loop_with_tenth_step_runs_eleven_times() {
    let (result, output) = run(&[
        "10 N% = 0",
        "20 FOR X = 0 TO 1 STEP 0.1",
        "30 N% = N% + 1",
        "40 NEXT X",
        "50 PRINT N%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "11\n");
}

#[test]
fn test_integer_loop_is_unchanged() {
    let (result, output) = run(&["10 FOR I% = 1 TO 3", "20 PRINT I%", "30 NEXT I%"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n2\n3\n");
}

#[test]
fn test_integer_loop_truncates_real_values() {
    let (result, output) = run(&["10 FOR I% = 1.7 TO 3.9", "20 PRINT I%", "30 NEXT I%"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n2\n3\n");
}

/// The step 0.5 is truncated to 0. A step of 0 keeps the behaviour that the
/// code had before: the loop runs one time.
#[test]
fn test_integer_loop_with_half_step_truncates_step_to_zero() {
    let (result, output) = run(&[
        "10 FOR I% = 1 TO 2 STEP 0.5",
        "20 PRINT I%",
        "30 NEXT I%",
        "40 PRINT \"END \"; I%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\nEND 1\n");
}

#[test]
fn test_nested_real_loops() {
    let (result, output) = run(&[
        "10 FOR I = 1 TO 1.5 STEP 0.5",
        "20 FOR J = 0 TO 0.5 STEP 0.25",
        "30 PRINT I; \" \"; J",
        "40 NEXT J",
        "50 NEXT I",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1 0\n1 0.25\n1 0.5\n1.5 0\n1.5 0.25\n1.5 0.5\n");
}

#[test]
fn test_real_loop_with_values_from_variables() {
    let (result, output) = run(&[
        "10 S = 0.5",
        "20 E = 1.5",
        "30 D = 0.5",
        "40 FOR I = S TO E STEP D",
        "50 PRINT I",
        "60 NEXT I",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0.5\n1\n1.5\n");
}

#[test]
fn test_real_loop_variable_after_loop_ends() {
    let (result, output) = run(&["10 FOR I = 1 TO 2 STEP 0.5", "20 NEXT I", "30 PRINT I"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "2.5\n");
}

#[test]
fn test_integer_loop_variable_after_loop_ends() {
    let (result, output) = run(&["10 FOR I% = 1 TO 3", "20 NEXT I%", "30 PRINT I%"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "4\n");
}
