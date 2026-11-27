//! Issue #180: a comment can follow the line number after THEN or ELSE.
//! The comment ends the branch, as the end of the line does.

use crate::common::run_with_limit as run;

#[test]
fn test_apostrophe_comment_after_then_line_number_runs() {
    let (result, output) = run(&["30 IF 1 THEN 50 ' d", "40 PRINT \"NO\"", "50 PRINT \"YES\""]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "YES\n");
}

#[test]
fn test_rem_comment_after_then_line_number_runs() {
    let (result, output) = run(&[
        "30 IF 1 THEN 50 REM d",
        "40 PRINT \"NO\"",
        "50 PRINT \"YES\"",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "YES\n");
}

#[test]
fn test_rem_comment_after_else_line_number_runs() {
    let (result, output) = run(&[
        "30 IF 0 THEN 50 ELSE 60 REM d",
        "40 PRINT \"NO\"",
        "50 PRINT \"THEN\"",
        "60 PRINT \"ELSE\"",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "ELSE\n");
}

#[test]
fn test_apostrophe_comment_after_else_line_number_runs() {
    let (result, output) = run(&[
        "30 IF 0 THEN 50 ELSE 60 ' d",
        "40 PRINT \"NO\"",
        "50 PRINT \"THEN\"",
        "60 PRINT \"ELSE\"",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "ELSE\n");
}

#[test]
fn test_calculation_after_then_line_number_keeps_its_error() {
    let (result, _) = run(&["30 IF 1 THEN 50 + 1", "50 PRINT \"YES\""]);
    assert_eq!(
        result,
        Err("Syntax error at line 30: Expected end of statement after line number".to_string())
    );
}
