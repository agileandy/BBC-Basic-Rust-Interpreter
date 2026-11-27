//! Issue 15: a scalar and an array with the same name are separate variables.

use crate::common::run_lines as run;

#[test]
fn test_integer_scalar_and_array_with_same_name_are_separate() {
    let (result, output) = run(&[
        "10 A% = 5",
        "20 DIM A%(3)",
        "30 A%(1) = 9",
        "40 PRINT A%",
        "50 PRINT A%(1)",
        "60 A% = 6",
        "70 PRINT A%(1)",
        "80 PRINT A%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         5\n         9\n         9\n         6\n");
}

#[test]
fn test_real_scalar_and_array_with_same_name_are_separate() {
    let (result, output) = run(&[
        "10 X = 2.5",
        "20 DIM X(3)",
        "30 X(2) = 7.5",
        "40 PRINT X",
        "50 PRINT X(2)",
        "60 X = 1.5",
        "70 PRINT X(2)",
        "80 PRINT X",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "       2.5\n       7.5\n       7.5\n       1.5\n");
}

#[test]
fn test_string_scalar_and_array_with_same_name_are_separate() {
    let (result, output) = run(&[
        "10 N$ = \"ONE\"",
        "20 DIM N$(3)",
        "30 N$(1) = \"TWO\"",
        "40 PRINT N$",
        "50 PRINT N$(1)",
        "60 N$ = \"SIX\"",
        "70 PRINT N$(1)",
        "80 PRINT N$",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "ONE\nTWO\nTWO\nSIX\n");
}

#[test]
fn test_local_scalar_leaves_array_readable_in_and_after_procedure() {
    let (result, output) = run(&[
        "10 A% = 5",
        "20 DIM A%(3)",
        "30 A%(1) = 9",
        "40 PROCCHANGE",
        "50 PRINT A%",
        "60 PRINT A%(1)",
        "70 END",
        "100 DEF PROCCHANGE",
        "110 LOCAL A%",
        "120 A% = 7",
        "130 PRINT A%",
        "140 PRINT A%(1)",
        "150 A%(1) = 8",
        "160 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         7\n         9\n         5\n         8\n");
}

#[test]
fn test_local_array_leaves_scalar_readable_in_and_after_procedure() {
    let (result, output) = run(&[
        "10 A% = 5",
        "20 DIM A%(3)",
        "30 A%(1) = 9",
        "40 PROCCHANGE",
        "50 PRINT A%",
        "60 PRINT A%(1)",
        "70 END",
        "100 DEF PROCCHANGE",
        "110 LOCAL A%()",
        "120 DIM A%(3)",
        "130 A%(1) = 4",
        "140 PRINT A%",
        "150 PRINT A%(1)",
        "160 A% = 6",
        "170 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         5\n         4\n         6\n         9\n");
}

#[test]
fn test_local_array_without_dim_gives_array_not_dimensioned() {
    let (result, output) = run(&[
        "10 DIM A%(3)",
        "20 PROCREAD",
        "30 END",
        "100 DEF PROCREAD",
        "110 LOCAL A%()",
        "120 PRINT A%(0)",
        "130 ENDPROC",
    ]);

    let message = result.expect_err("A%(0) must not be readable before DIM");
    // An array that has no DIM gives Array (issue #122).
    assert_eq!(message, "Array at line 120");
    assert_eq!(output, "");
}
