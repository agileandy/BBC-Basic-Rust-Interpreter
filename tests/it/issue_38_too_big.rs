//! Issue 38: a real value that is too large for an integer gives the "Too big" error.

use crate::common::run_with_replies;
use bbc_basic_interpreter::error::BBCBasicError;
use bbc_basic_interpreter::tokenizer::tokenize;

/// Store the source lines, run the program and return the result and the output.
fn run(source: &[&str]) -> (Result<(), String>, String) {
    run_with_replies(source, &[])
}

#[test]
fn test_large_real_into_integer_variable_gives_too_big() {
    let (result, output) = run(&["10 C% = 9000000000.0 / 2", "20 PRINT C%"]);

    assert_eq!(result, Err("Too big at line 10".to_string()));
    assert_eq!(output, "");
}

#[test]
fn test_large_real_literal_into_integer_variable_gives_too_big() {
    let (result, _) = run(&["10 A% = 3000000000"]);

    assert_eq!(result, Err("Too big at line 10".to_string()));
}

#[test]
fn test_large_real_variable_into_integer_variable_gives_too_big() {
    let (result, _) = run(&["10 X = 3E9", "20 A% = X"]);

    assert_eq!(result, Err("Too big at line 20".to_string()));
}

#[test]
fn test_large_real_array_element_into_integer_variable_gives_too_big() {
    let (result, _) = run(&["10 DIM R(1)", "20 R(1) = -3E9", "30 A% = R(1)"]);

    assert_eq!(result, Err("Too big at line 30".to_string()));
}

#[test]
fn test_largest_and_smallest_values_in_range_are_truncated() {
    let (result, output) = run(&[
        "10 A% = 2147483647.9",
        "20 B% = -2147483648.9",
        "30 PRINT A%",
        "40 PRINT B%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "2147483647\n-2147483648\n");
}

#[test]
fn test_large_input_reply_into_integer_variable_gives_too_big() {
    let (result, _) = run_with_replies(&["10 INPUT A%", "20 PRINT A%"], &["3000000000"]);

    assert_eq!(result, Err("Too big at line 10".to_string()));
}

#[test]
fn test_val_of_large_number_in_integer_context_gives_too_big() {
    let (result, _) = run(&["10 A% = VAL(\"3000000000\")"]);

    assert_eq!(result, Err("Too big at line 10".to_string()));
}

#[test]
fn test_on_error_catches_too_big_and_err_is_20() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 A% = 3E9",
        "30 END",
        "100 PRINT ERR",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "20\n");
}

#[test]
fn test_real_literal_that_is_not_finite_gives_too_big() {
    assert_eq!(
        tokenize("10 X = 1E400").map(|_| ()),
        Err(BBCBasicError::TooBig)
    );
}
