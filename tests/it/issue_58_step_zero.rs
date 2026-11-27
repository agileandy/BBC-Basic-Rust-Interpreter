//! Issue 58: a FOR loop with a step of 0 uses the end test for a positive step.
//!
//! The loop ends when the loop variable is larger than the end value. If the
//! body does not change the loop variable, a loop with a start value that is
//! not larger than the end value does not end.

use crate::common::run_timed_the_program_stops as run;

#[test]
fn test_step_zero_start_below_end_loops_until_body_passes_end() {
    let (result, output) = run(&[
        "10 FOR I = 1 TO 3 STEP 0",
        "20 PRINT I",
        "30 I = I + 1",
        "40 NEXT I",
        "50 PRINT \"DONE\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         1\n         2\n         3\nDONE\n");
}

#[test]
fn test_step_zero_integer_loop_start_below_end_loops_until_body_passes_end() {
    let (result, output) = run(&[
        "10 FOR I% = 1 TO 3 STEP 0",
        "20 PRINT I%",
        "30 I% = I% + 1",
        "40 NEXT I%",
        "50 PRINT \"DONE\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         1\n         2\n         3\nDONE\n");
}

#[test]
fn test_step_zero_start_above_end_runs_one_pass() {
    let (result, output) = run(&[
        "10 FOR I = 5 TO 3 STEP 0",
        "20 PRINT I",
        "30 NEXT I",
        "40 PRINT \"DONE\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         5\nDONE\n");
}

#[test]
fn test_step_zero_start_equal_to_end_ends_when_body_passes_end() {
    let (result, output) = run(&[
        "10 FOR I = 3 TO 3 STEP 0",
        "20 PRINT I",
        "30 I = I + 1",
        "40 NEXT I",
        "50 PRINT \"DONE\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         3\nDONE\n");
}

#[test]
fn test_positive_step_is_unchanged() {
    let (result, output) = run(&["10 FOR I = 1 TO 3 STEP 1", "20 PRINT I", "30 NEXT I"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         1\n         2\n         3\n");
}

#[test]
fn test_negative_step_is_unchanged() {
    let (result, output) = run(&["10 FOR I = 3 TO 1 STEP -1", "20 PRINT I", "30 NEXT I"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         3\n         2\n         1\n");
}
