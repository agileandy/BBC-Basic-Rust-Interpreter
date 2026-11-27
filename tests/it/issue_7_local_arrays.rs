//! Issue 7: a procedure must not change an array that it declared LOCAL.
//!
//! The programs use one statement per line and only indices 0 and 1 of a
//! DIM X(2) array, so that they do not depend on colon or DIM size handling.

use crate::common::run_lines as run;

#[test]
fn test_local_array_is_restored_after_endproc() {
    let (result, output) = run(&[
        "10 DIM A%(2)",
        "20 A%(0) = 1",
        "25 A%(1) = 2",
        "30 PROCCHANGE",
        "40 PRINT A%(0);A%(1)",
        "50 END",
        "100 DEF PROCCHANGE",
        "110 LOCAL A%()",
        "120 DIM A%(2)",
        "130 A%(0) = 7",
        "135 A%(1) = 8",
        "140 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         12\n");
}

#[test]
fn test_nested_local_arrays_are_each_restored() {
    let (result, output) = run(&[
        "10 DIM A%(2)",
        "20 A%(0) = 1",
        "25 A%(1) = 2",
        "30 PROCOUTER",
        "40 PRINT A%(0);A%(1)",
        "50 END",
        "100 DEF PROCOUTER",
        "110 LOCAL A%()",
        "120 DIM A%(2)",
        "130 A%(0) = 5",
        "135 A%(1) = 6",
        "140 PROCINNER",
        "150 PRINT A%(0);A%(1)",
        "160 ENDPROC",
        "200 DEF PROCINNER",
        "210 LOCAL A%()",
        "220 DIM A%(2)",
        "230 A%(0) = 8",
        "235 A%(1) = 9",
        "240 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         56\n         12\n");
}

#[test]
fn test_local_string_array_is_restored_after_endproc() {
    let (result, output) = run(&[
        "10 DIM N$(2)",
        "20 N$(0) = \"AB\"",
        "25 N$(1) = \"CD\"",
        "30 PROCCHANGE",
        "40 PRINT N$(0);N$(1)",
        "50 END",
        "100 DEF PROCCHANGE",
        "110 LOCAL N$()",
        "120 DIM N$(2)",
        "130 N$(0) = \"XY\"",
        "135 N$(1) = \"ZZ\"",
        "140 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "ABCD\n");
}

#[test]
fn test_procedure_with_local_array_runs_two_times() {
    let (result, output) = run(&[
        "10 PROCFILL",
        "20 PROCFILL",
        "30 PRINT \"DONE\"",
        "40 END",
        "100 DEF PROCFILL",
        "110 LOCAL B%()",
        "120 DIM B%(2)",
        "130 B%(1) = 4",
        "140 PRINT B%(1)",
        "150 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         4\n         4\nDONE\n");
}

#[test]
fn test_local_array_that_did_not_exist_is_removed() {
    let (result, output) = run(&[
        "10 PROCFILL",
        "20 PRINT B%(0)",
        "30 END",
        "100 DEF PROCFILL",
        "110 LOCAL B%()",
        "120 DIM B%(2)",
        "130 ENDPROC",
    ]);

    assert!(result.is_err(), "B%() must not exist after ENDPROC");
    assert_eq!(output, "");
}

#[test]
fn test_local_scalar_name_does_not_destroy_array() {
    let (result, output) = run(&[
        "10 DIM A%(2)",
        "20 A%(0) = 1",
        "25 A%(1) = 2",
        "30 PROCCHANGE",
        "40 PRINT A%(0);A%(1)",
        "50 END",
        "100 DEF PROCCHANGE",
        "110 LOCAL A%",
        "120 A% = 7",
        "130 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         12\n");
}
