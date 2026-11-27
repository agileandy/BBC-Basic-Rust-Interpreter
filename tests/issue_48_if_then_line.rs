//! Issue 48: a line number after THEN or ELSE is a short form of GOTO.

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
fn test_if_true_then_line_number_jumps() {
    let (result, output) = run(&[
        "10 X = 1",
        "20 IF X THEN 40",
        "30 PRINT \"SKIPPED\"",
        "40 PRINT \"TARGET\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "TARGET\n");
}

#[test]
fn test_if_false_then_line_number_continues_with_next_line() {
    let (result, output) = run(&[
        "10 X = 0",
        "20 IF X THEN 40",
        "30 PRINT \"NEXT\"",
        "40 PRINT \"TARGET\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "NEXT\nTARGET\n");
}

#[test]
fn test_if_then_line_number_else_line_number() {
    let program = |x: &'static str| {
        run(&[
            x,
            "20 IF X THEN 50 ELSE 70",
            "30 PRINT \"SKIPPED\"",
            "40 END",
            "50 PRINT \"THEN\"",
            "60 END",
            "70 PRINT \"ELSE\"",
        ])
    };

    assert_eq!(program("10 X = 1"), (Ok(()), "THEN\n".to_string()));
    assert_eq!(program("10 X = 0"), (Ok(()), "ELSE\n".to_string()));
}

#[test]
fn test_if_then_print_else_line_number() {
    let program = |x: &'static str| {
        run(&[
            x,
            "20 IF X THEN PRINT \"A\" ELSE 50",
            "30 PRINT \"NEXT\"",
            "40 END",
            "50 PRINT \"ELSE\"",
        ])
    };

    assert_eq!(program("10 X = 1"), (Ok(()), "A\nNEXT\n".to_string()));
    assert_eq!(program("10 X = 0"), (Ok(()), "ELSE\n".to_string()));
}

#[test]
fn test_if_then_line_number_else_print() {
    let program = |x: &'static str| {
        run(&[
            x,
            "20 IF X THEN 50 ELSE PRINT \"B\"",
            "30 PRINT \"NEXT\"",
            "40 END",
            "50 PRINT \"THEN\"",
        ])
    };

    assert_eq!(program("10 X = 1"), (Ok(()), "THEN\n".to_string()));
    assert_eq!(program("10 X = 0"), (Ok(()), "B\nNEXT\n".to_string()));
}

#[test]
fn test_if_then_missing_line_gives_goto_error() {
    let (result, _) = run(&["10 IF 1 THEN 99"]);

    assert_eq!(result, Err("Line 99 not found (GOTO)".to_string()));
}

#[test]
fn test_if_then_line_number_jumps_out_of_for_body_each_time() {
    let (result, output) = run(&[
        "10 FOR I% = 1 TO 3",
        "20 IF I% > 0 THEN 40",
        "30 PRINT \"SKIPPED\"",
        "40 PRINT I%",
        "50 NEXT I%",
        "60 PRINT \"DONE\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n2\n3\nDONE\n");
}

#[test]
fn test_if_then_line_number_jumps_out_of_inner_for_loop_3_times() {
    let (result, output) = run(&[
        "10 FOR I% = 1 TO 3",
        "20 FOR J% = 1 TO 10",
        "30 IF J% = 2 THEN 60",
        "40 NEXT J%",
        "60 PRINT I% * 10 + J%",
        "70 NEXT I%",
        "80 PRINT \"DONE\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "12\n22\n32\nDONE\n");
}
