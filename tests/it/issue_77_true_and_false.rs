//! Issue #77: TRUE and FALSE with no bracket give -1 and 0.

use crate::common::run_with_limit;

#[test]
fn test_print_true_shows_minus_one() {
    let (result, output) = run_with_limit(&["10 PRINT TRUE"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "-1");
}

#[test]
fn test_print_false_shows_zero() {
    let (result, output) = run_with_limit(&["10 PRINT FALSE"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_if_true_runs_then_part() {
    let (result, output) = run_with_limit(&["10 IF TRUE THEN PRINT \"T\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "T\n");
}

#[test]
fn test_repeat_until_with_or_false_ends() {
    let (result, output) = run_with_limit(&[
        "10 N% = 0",
        "20 REPEAT : N% = N% + 1 : UNTIL N% = 3 OR FALSE",
        "30 PRINT N%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "3");
}

/// The OR test above passes without the fix, because OR is not parsed (a
/// separate defect). This test makes the loop condition depend on FALSE.
#[test]
fn test_repeat_until_with_and_not_false_ends() {
    let (result, output) = run_with_limit(&[
        "10 N% = 0",
        "20 REPEAT : N% = N% + 1 : UNTIL N% = 3 AND NOT FALSE",
        "30 PRINT N%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "3");
}

#[test]
fn test_true_and_five_gives_five() {
    let (result, output) = run_with_limit(&["10 A% = TRUE AND 5", "20 PRINT A%"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_not_false_variable_runs_then_part() {
    let (result, output) = run_with_limit(&["10 F% = FALSE : IF NOT F% THEN PRINT \"N\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "N\n");
}

#[test]
fn test_names_that_start_with_true_or_false_are_variables() {
    let (result, output) = run_with_limit(&[
        "10 TRUEVALUE = 5",
        "20 FALSEFLAG% = 7",
        "30 PRINT TRUEVALUE; \" \"; FALSEFLAG%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         5 7\n");
}
