//! Issue #80: OR and EOR give the BBC BASIC results.
//!
//! The parser mapped the keyword code of EOR to OR and did not know the code
//! of OR. OR ended the expression, and EOR gave the result of OR.

use crate::common::run_with_limit;

#[test]
fn test_zero_or_five_is_five() {
    let (result, output) = run_with_limit(&["10 PRINT 0 OR 5"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_six_eor_three_is_five() {
    let (result, output) = run_with_limit(&["10 PRINT 6 EOR 3"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_six_and_three_is_two() {
    let (result, output) = run_with_limit(&["10 PRINT 6 AND 3"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "2");
}

#[test]
fn test_and_binds_more_than_or() {
    // 1 OR (2 AND 4) is 1. (1 OR 2) AND 4 is 0.
    let (result, output) = run_with_limit(&["10 PRINT 1 OR 2 AND 4"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_if_zero_or_one_takes_then_branch() {
    let (result, output) = run_with_limit(&["10 IF 0 OR 1 THEN PRINT \"Y\" ELSE PRINT \"N\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "Y\n");
}

#[test]
fn test_until_with_or_false_ends_at_three() {
    let (result, output) = run_with_limit(&[
        "10 A% = 0",
        "20 REPEAT",
        "30 A% = A% + 1",
        "40 UNTIL A% = 3 OR FALSE",
        "50 PRINT A%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "3");
}

#[test]
fn test_two_string_comparisons_joined_by_or() {
    let (result, output) = run_with_limit(&[
        "10 A$ = \"Y\"",
        "20 IF A$ = \"X\" OR A$ = \"Y\" THEN PRINT \"MATCH\" ELSE PRINT \"NO\"",
        "30 A$ = \"Z\"",
        "40 IF A$ = \"X\" OR A$ = \"Y\" THEN PRINT \"MATCH\" ELSE PRINT \"NO\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "MATCH\nNO\n");
}

#[test]
fn test_not_zero_or_zero_is_minus_one() {
    let (result, output) = run_with_limit(&["10 PRINT NOT 0 OR 0"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "-1");
}
