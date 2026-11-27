//! Issue 88: text that is left after an expression gives a syntax error.
//!
//! Before this change, the parser stopped at the first token that did not
//! continue the expression and ignored the rest of the statement.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::common::run_timed_stops as run;
use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_typed_line;
use bbc_basic_interpreter::tokenizer::tokenize;

/// Run the program and check that it gives an error at the line and prints nothing.
fn check_error(source: &[&str], line: &str) {
    let (result, output) = run(source);
    let message = result.expect_err(&format!("program gives an error: {:?}", source));
    assert!(
        message.contains(&format!("at line {}", line)),
        "program: {:?}, message: {}",
        source,
        message
    );
    assert_eq!(output, "", "program: {:?}", source);
}

#[test]
fn test_assignment_with_two_numbers() {
    check_error(&["10 A = 1 2", "20 PRINT \"RAN\""], "10");
}

#[test]
fn test_if_condition_with_two_numbers() {
    check_error(&["10 IF 1 2 THEN PRINT 1", "20 PRINT \"RAN\""], "10");
}

#[test]
fn test_while_condition_with_two_items() {
    check_error(&["10 WHILE X 2", "20 PRINT \"RAN\"", "30 ENDWHILE"], "10");
}

#[test]
fn test_until_condition_with_two_numbers() {
    check_error(&["10 REPEAT", "20 UNTIL 1 2", "30 PRINT \"RAN\""], "20");
}

#[test]
fn test_for_limit_with_two_numbers() {
    check_error(
        &["10 FOR I = 1 TO 3 4", "20 PRINT \"RAN\"", "30 NEXT I"],
        "10",
    );
}

#[test]
fn test_dim_size_with_two_numbers() {
    check_error(&["10 DIM A(2 3)", "20 PRINT \"RAN\""], "10");
}

#[test]
fn test_bare_rnd_before_minus() {
    check_error(&["10 X = RND-1", "20 PRINT \"RAN\""], "10");
}

#[test]
fn test_correct_statements_still_run() {
    let (result, output) = run(&[
        "10 A = 1 + 2",
        "20 IF A = 3 THEN PRINT \"YES\"",
        "30 FOR I = 1 TO 2 : PRINT I : NEXT I",
        "40 DIM B(2 + 1)",
        "50 B(1) = -(A)",
        "60 PRINT B(1) + A",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "YES\n1\n2\n0\n");
}

/// Run one typed line with no stored program and return the result and the output.
fn run_typed(text: &str) -> (Result<(), String>, String) {
    let text = text.to_string();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut executor = Executor::new();
        let program = ProgramStore::new();
        let line = tokenize(&text).expect("typed line tokenizes");
        let result = run_typed_line(&mut executor, &program, &line).map(|_| ());
        let _ = sender.send((result, executor.get_output().to_string()));
    });
    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("typed line stops in 5 seconds")
}

#[test]
fn test_typed_assignment_with_two_numbers() {
    let (result, output) = run_typed("A = 1 2");
    assert!(result.is_err(), "{result:?}");
    assert_eq!(output, "");
}

#[test]
fn test_typed_repeat_until_with_two_numbers() {
    let (result, output) = run_typed("REPEAT UNTIL 1 2");
    assert!(result.is_err(), "{result:?}");
    assert_eq!(output, "");
}

#[test]
fn test_stored_repeat_until_with_two_numbers() {
    check_error(&["10 REPEAT UNTIL 1 2", "20 PRINT \"RAN\""], "10");
}

#[test]
fn test_statement_after_repeat_with_no_colon_runs() {
    let (result, output) = run(&[
        "10 I% = 0",
        "20 REPEAT I% = I% + 1 : UNTIL I% = 3",
        "30 PRINT I%",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "3\n");
}

#[test]
fn test_memory_forms_still_run() {
    let (result, output) = run(&[
        "10 A% = &3000",
        "20 ?A% = 300",
        "30 A%?1 = 7",
        "40 PRINT A%?1 + 2",
        "50 A%!4 = -5",
        "60 PRINT !(A% + 4)",
        "70 PRINT ?A%",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "9\n-5\n44\n");
}

#[test]
fn test_memory_write_with_two_numbers() {
    check_error(&["10 A% = &3000", "20 ?A% = 1 2", "30 PRINT \"RAN\""], "20");
}

#[test]
fn test_end_with_text_after_it() {
    check_error(&["10 PRINT \"RAN\"", "20 END 1"], "20");
}

#[test]
fn test_stop_with_text_after_it() {
    check_error(&["10 PRINT \"RAN\"", "20 STOP 1"], "20");
}

#[test]
fn test_cls_with_text_after_it() {
    check_error(&["10 CLS 1", "20 PRINT \"RAN\""], "10");
}

#[test]
fn test_clear_with_text_after_it() {
    check_error(&["10 CLEAR 1", "20 PRINT \"RAN\""], "10");
}

#[test]
fn test_statements_with_no_argument_still_run() {
    let (result, output) = run(&[
        "10 CLS : CLEAR : PRINT \"A\"",
        "20 IF 1 THEN END ELSE PRINT \"B\"",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "A\n");
}
