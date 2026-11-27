//! Issue 59: text after a line number in a THEN or an ELSE branch is an error.

use crate::common::run_timed_stops as run;

#[test]
fn test_if_then_line_number_with_calculation_does_not_start() {
    let (result, output) = run(&[
        "10 PRINT \"START\"",
        "20 IF 1 THEN 30+1",
        "30 PRINT \"THIRTY\"",
    ]);

    let error = result.expect_err("program does not start");
    assert!(
        error.contains("Expected end of statement after line number"),
        "error was {error}"
    );
    assert!(error.contains("line 20"), "error was {error}");
    assert_eq!(output, "");
}

#[test]
fn test_if_else_line_number_with_calculation_does_not_start() {
    let (result, output) = run(&[
        "10 PRINT \"START\"",
        "20 IF 0 THEN 30 ELSE 30+1",
        "30 PRINT \"THIRTY\"",
    ]);

    let error = result.expect_err("program does not start");
    assert!(
        error.contains("Expected end of statement after line number"),
        "error was {error}"
    );
    assert_eq!(output, "");
}

#[test]
fn test_if_then_line_number_colon_print_still_jumps() {
    let (result, output) = run(&[
        "10 IF 1 THEN 30 : PRINT 1",
        "20 PRINT \"SKIPPED\"",
        "30 PRINT \"THIRTY\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "THIRTY\n");
}
