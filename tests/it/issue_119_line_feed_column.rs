//! Issue #119: a line feed (code 10) keeps the print column. A carriage
//! return (code 13) and the new line of PRINT set the column to 0.

use crate::common::run_with_limit as run;

#[test]
fn test_vdu_10_keeps_the_column() {
    let (result, output) = run(&["10 PRINT \"ABC\"; : VDU 10 : PRINT \"D\"; : PRINT POS"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "ABC\nD         4\n");
}

#[test]
fn test_vdu_13_sets_the_column_to_0() {
    let (result, output) = run(&["10 PRINT \"ABC\"; : VDU 13 : P% = POS : PRINT P%"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "ABC\r         0\n");
}

#[test]
fn test_print_new_line_sets_the_column_to_0() {
    let (result, output) = run(&["10 PRINT \"ABC\"", "20 PRINT POS"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "ABC\n         0\n");
}

#[test]
fn test_chr_10_in_print_keeps_the_column() {
    let (result, output) = run(&["10 PRINT \"AB\"; CHR$(10); \"C\"; : PRINT POS"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "AB\nC         3\n");
}

#[test]
fn test_line_feed_in_a_string_keeps_the_column() {
    let (result, output) = run(&[
        "10 A$ = \"AB\" + CHR$(10) + \"C\"",
        "20 PRINT A$; : PRINT POS",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "AB\nC         3\n");
}

#[test]
fn test_chr_13_in_print_sets_the_column_to_0() {
    let (result, output) = run(&["10 PRINT \"AB\"; CHR$(13); \"C\"; : PRINT POS"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "AB\rC         1\n");
}

#[test]
fn test_tab_to_an_earlier_column_starts_a_new_line_at_column_0() {
    let (result, output) = run(&["10 PRINT \"ABCDE\"; TAB(2); \"X\"; : PRINT POS"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "ABCDE\n  X         3\n");
}
