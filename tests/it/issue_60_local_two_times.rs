//! Issue 60: two LOCAL statements for one name restore the value from before the call.

use crate::common::run_lines as run;

#[test]
fn test_integer_local_two_times_restores_first_value() {
    let (result, output) = run(&[
        "10 A% = 1",
        "20 PROCTEST",
        "30 PRINT A%",
        "40 END",
        "100 DEF PROCTEST",
        "110 LOCAL A%",
        "120 A% = 2",
        "130 LOCAL A%",
        "140 PRINT A%",
        "150 A% = 3",
        "160 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0\n1\n");
}

#[test]
fn test_real_local_two_times_restores_first_value() {
    let (result, output) = run(&[
        "10 X = 1.5",
        "20 PROCTEST",
        "30 PRINT X",
        "40 END",
        "100 DEF PROCTEST",
        "110 LOCAL X",
        "120 X = 2.5",
        "130 LOCAL X",
        "140 X = 3.5",
        "150 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1.5\n");
}

#[test]
fn test_string_local_two_times_restores_first_value() {
    let (result, output) = run(&[
        "10 N$ = \"ONE\"",
        "20 PROCTEST",
        "30 PRINT N$",
        "40 END",
        "100 DEF PROCTEST",
        "110 LOCAL N$",
        "120 N$ = \"TWO\"",
        "130 LOCAL N$",
        "140 N$ = \"THREE\"",
        "150 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "ONE\n");
}

#[test]
fn test_array_local_two_times_restores_first_array() {
    let (result, output) = run(&[
        "10 DIM A%(2)",
        "20 A%(0) = 1",
        "30 PROCTEST",
        "40 PRINT A%(0)",
        "50 END",
        "100 DEF PROCTEST",
        "110 LOCAL A%()",
        "120 DIM A%(2)",
        "130 A%(0) = 2",
        "140 LOCAL A%()",
        "150 DIM A%(2)",
        "160 A%(0) = 3",
        "170 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n");
}

#[test]
fn test_nested_call_with_local_two_times_restores_each_value() {
    let (result, output) = run(&[
        "10 A% = 1",
        "20 PROCOUTER",
        "30 PRINT A%",
        "40 END",
        "100 DEF PROCOUTER",
        "110 LOCAL A%",
        "120 A% = 2",
        "130 LOCAL A%",
        "140 A% = 3",
        "150 PROCINNER",
        "160 PRINT A%",
        "170 ENDPROC",
        "200 DEF PROCINNER",
        "210 LOCAL A%",
        "220 A% = 4",
        "230 LOCAL A%",
        "240 A% = 5",
        "250 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "3\n1\n");
}
