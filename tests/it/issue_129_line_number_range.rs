//! Tests that a line number above 65535 gives an error and does not change
//! to a different line (issue #129).

use crate::common::run_with_limit as run;

const OUT_OF_RANGE: &str = "Syntax error at line 10: Line number out of range";

/// Run a program whose line 10 has a literal line number that is too large.
/// Line 100 prints "wrapped" if the number changes to 100.
fn assert_out_of_range(line_10: &str) {
    let (result, output) = run(&[
        line_10,
        "20 PRINT 1/0",
        "100 PRINT \"wrapped\"",
        "110 DATA 7",
    ]);
    assert_eq!(result, Err(OUT_OF_RANGE.to_string()), "{line_10}");
    assert!(!output.contains("wrapped"), "{line_10}: {output}");
}

#[test]
fn on_error_goto_above_65535_is_a_syntax_error() {
    assert_out_of_range("10 ON ERROR GOTO 65636");
}

#[test]
fn goto_above_65535_is_a_syntax_error() {
    assert_out_of_range("10 GOTO 65636");
}

#[test]
fn gosub_above_65535_is_a_syntax_error() {
    assert_out_of_range("10 GOSUB 70000");
}

#[test]
fn restore_above_65535_is_a_syntax_error() {
    assert_out_of_range("10 RESTORE 65636");
}

#[test]
fn if_then_line_above_65535_is_a_syntax_error() {
    assert_out_of_range("10 IF 1 THEN 65636");
}

#[test]
fn if_else_line_above_65535_is_a_syntax_error() {
    assert_out_of_range("10 IF 0 THEN PRINT \"no\" ELSE 65636");
}

#[test]
fn on_goto_line_above_65535_is_a_syntax_error() {
    assert_out_of_range("10 ON 1 GOTO 65636");
}

#[test]
fn on_gosub_line_above_65535_is_a_syntax_error() {
    assert_out_of_range("10 ON 1 GOSUB 30, 65636");
}

#[test]
fn the_error_comes_before_the_run() {
    let (result, output) = run(&[
        "5 PRINT \"started\"",
        "10 GOTO 65636",
        "100 PRINT \"wrapped\"",
    ]);
    assert_eq!(result, Err(OUT_OF_RANGE.to_string()));
    assert_eq!(output, "");
}

#[test]
fn goto_65535_goes_to_line_65535() {
    let (result, output) = run(&["10 GOTO 65535", "20 PRINT \"no\"", "65535 PRINT \"top\""]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "top\n");
}

#[test]
fn restore_65535_reads_the_data_on_line_65535() {
    let (result, output) = run(&[
        "10 RESTORE 65535",
        "20 READ A",
        "30 PRINT A",
        "40 END",
        "50 DATA 1",
        "65535 DATA 2",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "2\n");
}

#[test]
fn a_negative_calculated_target_is_no_such_line() {
    let (result, output) = run(&["10 X = -1", "20 GOTO X", "100 PRINT \"wrapped\""]);
    assert_eq!(result, Err("No such line: -1 at line 20".to_string()));
    assert_eq!(output, "");
}

#[test]
fn a_large_calculated_target_is_no_such_line() {
    let (result, output) = run(&["10 X = 65636", "20 GOSUB X", "100 PRINT \"wrapped\""]);
    assert_eq!(result, Err("No such line: 65636 at line 20".to_string()));
    assert_eq!(output, "");
}

#[test]
fn on_error_catches_a_calculated_target_out_of_range() {
    let (result, output) = run(&[
        "10 ON ERROR PRINT ERR; \" \"; ERL : END",
        "20 X = 70000 : GOTO X",
        "100 PRINT \"wrapped\"",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "41 20\n");
}
