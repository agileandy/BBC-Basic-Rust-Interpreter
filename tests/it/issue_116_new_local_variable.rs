//! Tests that a LOCAL variable or a parameter with a name that did not exist
//! before the PROC or FN has the value 0, or an empty string, after the end
//! of the PROC or FN, as on a BBC Micro. A LOCAL array with a new name does
//! not exist after the end (issue #116).

use crate::common::run_timed_ran_for as run;

#[test]
fn test_new_local_real_is_zero_after_endproc() {
    let (result, output) = run(&[
        "10 PROCa",
        "20 PRINT Q",
        "30 END",
        "40 DEF PROCa",
        "50 LOCAL Q : Q = 5",
        "60 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0\n");
}

#[test]
fn test_new_local_string_is_empty_after_endproc() {
    let (result, output) = run(&[
        "10 PROCa",
        "20 PRINT \"[\"; Q$; \"]\"",
        "30 END",
        "40 DEF PROCa",
        "50 LOCAL Q$ : Q$ = \"IN\"",
        "60 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "[]\n");
}

#[test]
fn test_new_local_integer_is_zero_after_endproc() {
    let (result, output) = run(&[
        "10 PROCa",
        "20 PRINT Q%",
        "30 END",
        "40 DEF PROCa",
        "50 LOCAL Q% : Q% = 5",
        "60 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0\n");
}

#[test]
fn test_new_local_in_fn_is_zero_after_fn() {
    let (result, output) = run(&[
        "10 R = FNa",
        "20 PRINT R; \" \"; Q",
        "30 END",
        "40 DEF FNa",
        "50 LOCAL Q : Q = 5",
        "60 = Q + 1",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "6 0\n");
}

#[test]
fn test_new_parameter_is_zero_after_endproc() {
    let (result, output) = run(&[
        "10 PROCa(5, \"IN\")",
        "20 PRINT P; \"[\"; P$; \"]\"",
        "30 END",
        "40 DEF PROCa(P, P$)",
        "50 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0[]\n");
}

#[test]
fn test_new_local_array_does_not_exist_after_endproc() {
    let (result, output) = run(&[
        "10 PROCa",
        "20 DIM L(3) : L(3) = 9",
        "30 PRINT L(3)",
        "40 END",
        "50 DEF PROCa",
        "60 LOCAL L() : DIM L(1) : L(1) = 5",
        "70 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "9\n");
}

#[test]
fn test_read_of_new_local_array_after_endproc_gives_no_dim_error() {
    let (local_result, _) = run(&[
        "10 PROCa",
        "20 PRINT L(1)",
        "30 END",
        "40 DEF PROCa",
        "50 LOCAL L() : DIM L(1)",
        "60 ENDPROC",
    ]);
    let (no_dim_result, _) = run(&["10 PRINT L(1)", "20 END"]);

    assert_eq!(
        local_result,
        Err("No such variable: L at line 20".to_string())
    );
    assert_eq!(
        no_dim_result,
        Err("No such variable: L at line 10".to_string())
    );
}

#[test]
fn test_new_local_is_zero_after_trapped_error() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 PROCa",
        "30 END",
        "40 DEF PROCa",
        "50 LOCAL Q, Q$ : Q = 5 : Q$ = \"IN\"",
        "60 PRINT 1/0",
        "70 ENDPROC",
        "100 PRINT Q; \"[\"; Q$; \"]\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0[]\n");
}

#[test]
fn test_old_variable_value_still_restored() {
    let (result, output) = run(&[
        "10 Q = 7 : PROCa",
        "20 PRINT Q",
        "30 END",
        "40 DEF PROCa",
        "50 LOCAL Q : Q = 5",
        "60 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "7\n");
}
