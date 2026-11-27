//! Issue #165: an error in the body of a single-line function has the line
//! of its DEF, as on a BBC Micro. An error in an argument of the call has
//! the line of the caller.

use crate::common::run_with_limit as run;

#[test]
fn test_error_in_single_line_function_gives_def_line() {
    let (result, _) = run(&["10 PRINT FNa", "20 END", "30 DEF FNa = 1 / 0"]);

    assert_eq!(result, Err("Division by zero at line 30".to_string()));
}

#[test]
fn test_erl_is_def_line_with_handler() {
    let (result, output) = run(&[
        "10 ON ERROR PRINT ERL : END",
        "20 PRINT FNa",
        "30 END",
        "40 DEF FNa = 1 / 0",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "        40\n");
}

#[test]
fn test_error_in_argument_gives_caller_line() {
    let (result, _) = run(&["10 PRINT FNa(1 / 0)", "20 END", "30 DEF FNa(X) = X"]);

    assert_eq!(result, Err("Division by zero at line 10".to_string()));
}

#[test]
fn test_nested_single_line_functions_give_innermost_def_line() {
    let (result, _) = run(&[
        "10 PRINT FNa",
        "20 END",
        "30 DEF FNa = FNb + 1",
        "40 DEF FNb = 1 / 0",
    ]);

    assert_eq!(result, Err("Division by zero at line 40".to_string()));
}

#[test]
fn test_error_in_argument_of_inner_call_gives_outer_def_line() {
    let (result, _) = run(&[
        "10 PRINT FNa",
        "20 END",
        "30 DEF FNa = FNb(1 / 0)",
        "40 DEF FNb(X) = X",
    ]);

    assert_eq!(result, Err("Division by zero at line 30".to_string()));
}

#[test]
fn test_def_before_the_call_gives_def_line() {
    // The run goes through the DEF line before the call
    let (result, _) = run(&["10 DEF FNa = 1 / 0", "20 PRINT FNa"]);

    assert_eq!(result, Err("Division by zero at line 10".to_string()));
}

#[test]
fn test_multi_line_function_is_unchanged() {
    let (result, _) = run(&[
        "10 PRINT FNa",
        "20 END",
        "30 DEF FNa",
        "40 X = 1 / 0",
        "50 = X",
    ]);

    assert_eq!(result, Err("Division by zero at line 40".to_string()));
}

#[test]
fn test_single_line_function_in_multi_line_body_gives_def_line() {
    let (result, _) = run(&[
        "10 PRINT FNa",
        "20 END",
        "30 DEF FNa",
        "40 X = FNb",
        "50 = X",
        "60 DEF FNb = 1 / 0",
    ]);

    assert_eq!(result, Err("Division by zero at line 60".to_string()));
}

#[test]
fn test_multi_line_function_in_single_line_body_gives_body_line() {
    let (result, _) = run(&[
        "10 PRINT FNa",
        "20 END",
        "30 DEF FNa = FNb + 1",
        "40 DEF FNb",
        "50 X = 1 / 0",
        "60 = X",
    ]);

    assert_eq!(result, Err("Division by zero at line 50".to_string()));
}

#[test]
fn test_later_error_after_good_call_gives_its_own_line() {
    let (result, output) = run(&["10 PRINT FNa", "20 X = 1 / 0", "30 END", "40 DEF FNa = 2"]);

    assert_eq!(result, Err("Division by zero at line 20".to_string()));
    assert_eq!(output, "         2\n");
}
