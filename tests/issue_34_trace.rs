//! Issue 34: TRACE shows the number of each line that runs.
//!
//! When trace is on, the run prints the line number in square brackets and
//! a space, for example `[20] `, each time it starts a stored line. `TRACE n`
//! traces only the lines with a number below n. The trace state stays on
//! between runs until `TRACE OFF`.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::{run_program, run_typed_line};
use bbc_basic_interpreter::tokenizer::tokenize;

/// Run the typed lines, then the stored program, on one executor. Return
/// the result of the program run and all the output.
///
/// The run has a time limit, so that a loop defect fails the test and does
/// not stop the test run.
fn run_after(typed: &[&str], source: &[&str]) -> (Result<(), String>, String) {
    let typed: Vec<String> = typed.iter().map(|line| line.to_string()).collect();
    let source: Vec<String> = source.iter().map(|line| line.to_string()).collect();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut executor = Executor::new();
        let mut program = ProgramStore::new();
        for line in &source {
            program.store_line(tokenize(line).expect("line tokenizes"));
        }
        for line in &typed {
            let line = tokenize(line).expect("typed line tokenizes");
            run_typed_line(&mut executor, &program, &line).expect("typed line runs");
        }
        let result = run_program(&mut executor, &mut program);
        let _ = sender.send((result, executor.get_output().to_string()));
    });
    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("the program stops in 5 seconds")
}

/// Store the source lines, run the program and return the result and the output.
fn run(source: &[&str]) -> (Result<(), String>, String) {
    run_after(&[], source)
}

#[test]
fn test_trace_on_prints_each_line_until_trace_off() {
    let (result, output) = run(&[
        "10 TRACE ON",
        "20 PRINT \"A\"",
        "30 TRACE OFF",
        "40 PRINT \"B\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "[20] A\n[30] B\n");
}

#[test]
fn test_for_loop_over_two_lines_traces_each_pass() {
    let (result, output) = run(&[
        "10 TRACE ON",
        "20 FOR I%=1 TO 2: PRINT \"X\"",
        "30 NEXT",
        "40 TRACE OFF",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "[20] X\n[30] [20] X\n[30] [40] ");
}

#[test]
fn test_trace_n_traces_only_lines_below_n() {
    let (result, output) = run(&[
        "10 TRACE 30",
        "20 PRINT \"A\"",
        "30 PRINT \"B\"",
        "40 PRINT \"C\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "[20] A\nB\nC\n");
}

#[test]
fn test_line_with_three_statements_prints_its_number_one_time() {
    let (result, output) = run(&[
        "10 TRACE ON",
        "20 PRINT \"A\";: PRINT \"B\";: PRINT \"C\"",
        "30 TRACE OFF",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "[20] ABC\n[30] ");
}

#[test]
fn test_jump_back_to_same_line_prints_again() {
    let (result, output) = run(&[
        "10 TRACE ON: I%=0",
        "20 I%=I%+1: IF I%<3 THEN 20",
        "30 TRACE OFF",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "[20] [20] [20] [30] ");
}

#[test]
fn test_typed_trace_on_stays_on_for_the_next_run() {
    let (result, output) = run_after(&["TRACE ON"], &["10 PRINT \"A\"", "20 TRACE OFF"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "[10] A\n[20] ");
}

#[test]
fn test_typed_trace_off_stops_the_trace() {
    let (result, output) = run_after(&["TRACE ON", "TRACE OFF"], &["10 PRINT \"A\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "A\n");
}
