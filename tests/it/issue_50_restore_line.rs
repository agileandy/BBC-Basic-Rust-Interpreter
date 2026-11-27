//! Tests for RESTORE with a line number (issue #50).

use crate::common::run_lines as run;

#[test]
fn test_restore_to_second_data_line_reads_its_first_value() {
    let (result, output) = run(&[
        "10 READ A%",
        "20 RESTORE 60",
        "30 READ B%",
        "40 PRINT A%;\" \";B%",
        "50 END",
        "55 DATA 1, 2",
        "60 DATA 3, 4",
        "70 DATA 5, 6",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         1 3\n");
}

#[test]
fn test_restore_on_same_line_as_read() {
    let (result, output) = run(&[
        "10 READ A% : RESTORE 30 : READ B%",
        "15 PRINT A%;\" \";B%",
        "16 END",
        "20 DATA 1",
        "30 DATA 2",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         1 2\n");
}

#[test]
fn test_restore_to_first_data_line_reads_all_values_again() {
    let (result, output) = run(&[
        "10 READ A%, B%, C%",
        "20 RESTORE 100",
        "30 READ D%, E%, F%",
        "40 PRINT A%;B%;C%;D%;E%;F%",
        "50 END",
        "100 DATA 1, 2",
        "110 DATA 3",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         123123\n");
}

#[test]
fn test_restore_with_no_line_number_is_unchanged() {
    let (result, output) = run(&[
        "10 READ A%, B%",
        "20 RESTORE",
        "30 READ C%",
        "40 PRINT A%;B%;C%",
        "50 END",
        "60 DATA 7",
        "70 DATA 8",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         787\n");
}

#[test]
fn test_restore_to_line_with_no_data_moves_to_next_data_line() {
    let (result, output) = run(&[
        "10 READ A%",
        "20 RESTORE 65",
        "30 READ B%",
        "40 PRINT A%;\" \";B%",
        "50 END",
        "60 DATA 1",
        "70 DATA 2",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         1 2\n");
}

#[test]
fn test_restore_after_last_data_line_gives_out_of_data_at_next_read() {
    let (result, output) = run(&[
        "10 RESTORE 100",
        "20 PRINT \"AFTER RESTORE\"",
        "30 READ A%",
        "40 END",
        "50 DATA 1",
    ]);

    let error = result.expect_err("READ after the last DATA fails");
    assert!(error.contains("line 30"), "error: {error}");
    assert!(error.contains("Out of DATA"), "error: {error}");
    assert_eq!(output, "AFTER RESTORE\n");
}

#[test]
fn test_restore_in_loop_reads_one_table_three_times() {
    // The FOR loop has a fixed count, so a defect gives a wrong output, not a hang
    let (result, output) = run(&[
        "10 FOR I% = 1 TO 3",
        "20 RESTORE 100",
        "30 READ A%, B%",
        "40 PRINT A%;B%",
        "50 NEXT I%",
        "60 END",
        "90 DATA 8, 9",
        "100 DATA 1, 2",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         12\n         12\n         12\n");
}
