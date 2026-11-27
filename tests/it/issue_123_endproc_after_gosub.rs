//! Tests that ENDPROC ends the procedure when a GOSUB of that procedure is
//! still open, and that RETURN does not return through a PROC call
//! (issue #123).

use crate::common::run_with_limit as run;

// D2: ENDPROC removes the open GOSUBs of the procedure

#[test]
fn test_endproc_in_subroutine_of_proc_ends_the_proc() {
    let (result, output) = run(&[
        "10 PROCp",
        "20 PRINT \"after proc\"",
        "30 END",
        "40 DEF PROCp",
        "50 GOSUB 100",
        "60 PRINT \"not here\"",
        "70 ENDPROC",
        "100 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "after proc\n");
}

#[test]
fn test_endproc_removes_two_open_gosubs_of_the_proc() {
    let (result, output) = run(&[
        "10 PROCp",
        "20 PRINT \"after proc\"",
        "30 END",
        "40 DEF PROCp",
        "50 GOSUB 100",
        "60 PRINT \"not here\"",
        "70 ENDPROC",
        "100 GOSUB 200",
        "110 PRINT \"not here either\"",
        "120 RETURN",
        "200 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "after proc\n");
}

#[test]
fn test_endproc_in_subroutine_restores_the_proc_parameter() {
    let (result, output) = run(&[
        "10 A = 1",
        "20 PROCp(2)",
        "30 PRINT A",
        "40 END",
        "50 DEF PROCp(A)",
        "60 GOSUB 100",
        "70 ENDPROC",
        "100 PRINT A",
        "110 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "2\n1\n");
}

// D3: RETURN with a PROC entry on top gives No GOSUB

#[test]
fn test_return_in_proc_with_no_gosub_gives_no_gosub() {
    let (result, output) = run(&[
        "10 PROCp",
        "20 PRINT \"after proc\"",
        "30 END",
        "40 DEF PROCp",
        "50 RETURN",
        "60 ENDPROC",
    ]);
    assert_eq!(result, Err("No GOSUB at line 50".to_string()));
    assert_eq!(output, "");
}

#[test]
fn test_return_in_proc_called_from_subroutine_gives_no_gosub() {
    let (result, output) = run(&[
        "10 GOSUB 100",
        "20 PRINT \"back\"",
        "30 END",
        "100 PROCp",
        "110 PRINT \"after proc\"",
        "120 RETURN",
        "200 DEF PROCp",
        "210 RETURN",
        "220 ENDPROC",
    ]);
    assert_eq!(result, Err("No GOSUB at line 210".to_string()));
    assert_eq!(output, "");
}

#[test]
fn test_return_in_proc_with_no_gosub_is_err_38() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 PROCp",
        "30 END",
        "40 DEF PROCp",
        "50 RETURN",
        "60 ENDPROC",
        "100 PRINT ERR; \" \"; ERL",
        "110 END",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "38 50\n");
}

// A GOSUB before the PROC call still returns after the procedure

#[test]
fn test_gosub_before_proc_call_returns_after_the_proc() {
    let (result, output) = run(&[
        "10 GOSUB 100",
        "20 PRINT \"back\"",
        "30 END",
        "100 PROCp",
        "110 PRINT \"after proc\"",
        "120 RETURN",
        "200 DEF PROCp",
        "210 GOSUB 300",
        "220 PRINT \"in proc\"",
        "230 ENDPROC",
        "300 PRINT \"sub\"",
        "310 RETURN",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "sub\nin proc\nafter proc\nback\n");
}

#[test]
fn test_gosub_before_proc_call_returns_after_endproc_in_subroutine() {
    let (result, output) = run(&[
        "10 GOSUB 100",
        "20 PRINT \"back\"",
        "30 END",
        "100 PROCp",
        "110 PRINT \"after proc\"",
        "120 RETURN",
        "200 DEF PROCp",
        "210 GOSUB 300",
        "220 PRINT \"not here\"",
        "230 ENDPROC",
        "300 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "after proc\nback\n");
}

// A recursive procedure with a GOSUB at each level

#[test]
fn test_recursive_proc_with_gosub_at_each_level() {
    let (result, output) = run(&[
        "10 PROCr(3)",
        "20 PRINT \"done\"",
        "30 END",
        "40 DEF PROCr(N)",
        "50 GOSUB 100",
        "60 PRINT \"not here\"",
        "70 ENDPROC",
        "100 PRINT N",
        "110 IF N > 1 THEN PROCr(N - 1)",
        "120 PRINT \"up \"; N",
        "130 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "3\n2\n1\nup 1\nup 2\nup 3\ndone\n");
}

#[test]
fn test_recursive_proc_with_gosub_that_returns_at_each_level() {
    let (result, output) = run(&[
        "10 PROCr(3)",
        "20 PRINT \"done\"",
        "30 END",
        "40 DEF PROCr(N)",
        "50 GOSUB 100",
        "60 IF N > 1 THEN PROCr(N - 1)",
        "70 PRINT \"up \"; N",
        "80 ENDPROC",
        "100 PRINT N",
        "110 RETURN",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "3\n2\n1\nup 1\nup 2\nup 3\ndone\n");
}

// D5: a GOSUB that is open at the `=` line of a function does not stay open

#[test]
fn test_gosub_open_at_fn_result_does_not_stay_on_the_stack() {
    let (result, output) = run(&[
        "10 X = FNf",
        "20 PRINT X",
        "30 RETURN",
        "100 DEF FNf",
        "110 GOSUB 200",
        "120 = 1",
        "200 = 2",
    ]);
    assert_eq!(result, Err("No GOSUB at line 30".to_string()));
    assert_eq!(output, "2\n");
}

#[test]
fn test_gosub_before_fn_call_returns_after_gosub_open_at_fn_result() {
    let (result, output) = run(&[
        "10 GOSUB 100",
        "20 PRINT \"back\"",
        "30 END",
        "100 X = FNf",
        "110 PRINT X",
        "120 RETURN",
        "200 DEF FNf",
        "210 GOSUB 300",
        "220 = 1",
        "300 = 2",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "2\nback\n");
}

#[test]
fn test_gosub_open_at_fn_result_in_proc_then_endproc() {
    let (result, output) = run(&[
        "10 PROCp",
        "20 PRINT \"after proc\"",
        "30 END",
        "40 DEF PROCp",
        "50 X = FNf",
        "60 PRINT X",
        "70 ENDPROC",
        "100 DEF FNf",
        "110 GOSUB 200",
        "120 = 1",
        "200 = 2",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "2\nafter proc\n");
}
