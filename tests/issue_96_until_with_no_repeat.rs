//! Tests that UNTIL with no open REPEAT loop, and ENDWHILE with no open
//! WHILE loop, stop with an error (issue #96).

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::{run_program, run_typed_line};
use bbc_basic_interpreter::tokenizer::tokenize;

/// Store the source lines, run the program and return the result and the
/// output. The run is in a thread with a time limit, so that a loop defect
/// fails the test and does not hang the test run.
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
        .expect("the program ran for more than 5 seconds")
}

/// Run one typed line with no stored program and return its error text.
fn typed_error_of(line: &str) -> String {
    let mut executor = Executor::new();
    let program = ProgramStore::new();
    let line = tokenize(line).expect("typed line tokenizes");
    run_typed_line(&mut executor, &program, &line).expect_err("the typed line gives an error")
}

// D1: UNTIL with no open REPEAT loop

#[test]
fn test_until_with_no_repeat_gives_no_repeat() {
    let (result, output) = run(&["10 UNTIL 1", "20 PRINT \"AFTER\""]);
    assert_eq!(result, Err("No REPEAT at line 10".to_string()));
    assert_eq!(output, "");
}

#[test]
fn test_until_false_with_no_repeat_gives_no_repeat() {
    let (result, _) = run(&["10 UNTIL 0", "20 PRINT \"AFTER\""]);
    assert_eq!(result, Err("No REPEAT at line 10".to_string()));
}

// D2: ON ERROR catches No REPEAT

#[test]
fn test_on_error_catches_no_repeat() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 UNTIL 1",
        "30 PRINT \"AFTER\"",
        "40 END",
        "100 PRINT ERR; \" \"; ERL",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "43 20\n");
}

#[test]
fn test_report_shows_no_repeat() {
    let (_, output) = run(&["10 ON ERROR GOTO 30", "20 UNTIL 1", "30 REPORT"]);
    assert_eq!(output, "No REPEAT");
}

// A correct REPEAT loop does not change

#[test]
fn test_repeat_loop_is_unchanged() {
    let (result, output) = run(&[
        "10 I% = 0",
        "20 REPEAT",
        "30 I% = I% + 1",
        "40 PRINT I%",
        "50 UNTIL I% = 3",
        "60 PRINT \"DONE\"",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n2\n3\nDONE\n");
}

#[test]
fn test_nested_repeat_loops_are_unchanged() {
    let (result, output) = run(&[
        "10 I% = 0",
        "20 REPEAT",
        "30 J% = 0",
        "40 REPEAT",
        "50 J% = J% + 1",
        "60 UNTIL J% = 2",
        "70 I% = I% + 1",
        "80 PRINT I%; \" \"; J%",
        "90 UNTIL I% = 2",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "1 2\n2 2\n");
}

#[test]
fn test_second_until_after_loop_ends_gives_no_repeat() {
    let (result, output) = run(&[
        "10 REPEAT",
        "20 PRINT \"IN\"",
        "30 UNTIL 1",
        "40 UNTIL 1",
        "50 PRINT \"AFTER\"",
    ]);
    assert_eq!(result, Err("No REPEAT at line 40".to_string()));
    assert_eq!(output, "IN\n");
}

// D4: a typed UNTIL with no open REPEAT loop

#[test]
fn test_typed_until_with_no_repeat_gives_no_repeat() {
    assert_eq!(typed_error_of("UNTIL 1"), "No REPEAT");
}

#[test]
fn test_typed_repeat_loop_is_unchanged() {
    let mut executor = Executor::new();
    let program = ProgramStore::new();
    let line = tokenize("I% = 0 : REPEAT : I% = I% + 1 : UNTIL I% = 3 : PRINT I%")
        .expect("typed line tokenizes");
    run_typed_line(&mut executor, &program, &line).expect("the typed line runs");
    assert_eq!(executor.get_output(), "3\n");
}

// D3: ENDWHILE with no open WHILE loop

#[test]
fn test_endwhile_with_no_while_gives_no_while() {
    let (result, output) = run(&["10 ENDWHILE", "20 PRINT \"AFTER\""]);
    assert_eq!(result, Err("No WHILE at line 10".to_string()));
    assert_eq!(output, "");
}

#[test]
fn test_on_error_catches_no_while() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 ENDWHILE",
        "30 PRINT \"AFTER\"",
        "40 END",
        "100 PRINT ERR; \" \"; ERL",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "46 20\n");
}

#[test]
fn test_typed_endwhile_with_no_while_gives_no_while() {
    assert_eq!(typed_error_of("ENDWHILE"), "No WHILE");
}

#[test]
fn test_while_loop_is_unchanged() {
    let (result, output) = run(&[
        "10 I% = 0",
        "20 WHILE I% < 3",
        "30 I% = I% + 1",
        "40 PRINT I%",
        "50 ENDWHILE",
        "60 PRINT \"DONE\"",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n2\n3\nDONE\n");
}
