//! Tests that ENDPROC in the body of a function, with no PROC open in that
//! body, gives No PROC and changes no scope and no stack of the caller
//! (issue #120).

use crate::common::run_with_limit as run;

// D1: ENDPROC in a function body gives No PROC

#[test]
fn test_endproc_in_fn_from_gosub_goes_to_handler_and_return_works() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 200",
        "20 GOSUB 100",
        "30 PRINT \"back\"",
        "40 END",
        "100 X = FNf",
        "110 PRINT \"after\"",
        "120 RETURN",
        "150 DEF FNf",
        "160 ENDPROC",
        "170 = 1",
        "200 PRINT ERR; \" \"; ERL",
        "210 GOSUB 300",
        "220 PRINT \"done\"",
        "230 END",
        "300 PRINT \"sub\"",
        "310 RETURN",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "13 160\nsub\ndone\n");
}

#[test]
fn test_endproc_in_fn_from_gosub_with_no_handler_gives_no_proc() {
    let (result, output) = run(&[
        "10 GOSUB 100",
        "20 PRINT \"back\"",
        "30 END",
        "100 X = FNf",
        "110 RETURN",
        "150 DEF FNf",
        "160 ENDPROC",
        "170 = 1",
    ]);
    assert_eq!(result, Err("No PROC at line 160".to_string()));
    assert_eq!(output, "");
}

#[test]
fn test_endproc_in_fn_from_proc_gives_no_proc() {
    let (result, output) = run(&[
        "10 PROCa",
        "20 PRINT \"back\"",
        "30 END",
        "100 DEF PROCa",
        "110 X = FNf",
        "120 PRINT \"proc\"",
        "130 ENDPROC",
        "150 DEF FNf",
        "160 ENDPROC",
        "170 = 1",
    ]);
    assert_eq!(result, Err("No PROC at line 160".to_string()));
    assert_eq!(output, "");
}

#[test]
fn test_endproc_in_fn_from_proc_in_fn_gives_no_proc() {
    // The PROC of the outer function is open, but the newest frame is the
    // inner function
    let (result, output) = run(&[
        "10 X = FNf",
        "20 PRINT \"back\"",
        "30 END",
        "100 DEF FNf",
        "110 PROCp",
        "120 = 1",
        "200 DEF PROCp",
        "210 Y = FNg",
        "220 PRINT \"proc\"",
        "230 ENDPROC",
        "300 DEF FNg",
        "310 ENDPROC",
        "320 = 2",
    ]);
    assert_eq!(result, Err("No PROC at line 310".to_string()));
    assert_eq!(output, "");
}

#[test]
fn test_endproc_in_fn_keeps_local_variable_of_caller_proc() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 200",
        "20 A = 1",
        "30 PROCa",
        "40 END",
        "100 DEF PROCa",
        "110 LOCAL A",
        "120 A = 2",
        "130 X = FNf",
        "140 ENDPROC",
        "150 DEF FNf",
        "160 ENDPROC",
        "170 = 1",
        "200 PRINT ERR; \" \"; A",
        "210 END",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "13 1\n");
}

// D2: a PROC that a function calls, and a function that a PROC calls, end
// correctly

#[test]
fn test_proc_called_from_fn_ends_correctly() {
    let (result, output) = run(&[
        "10 X = FNf(3)",
        "20 PRINT X",
        "30 END",
        "100 DEF FNf(N)",
        "110 PROCp(N)",
        "120 = R",
        "200 DEF PROCp(M)",
        "210 R = M * 2",
        "220 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "6\n");
}

#[test]
fn test_fn_called_from_proc_ends_correctly() {
    let (result, output) = run(&[
        "10 PROCa",
        "20 PRINT \"back\"",
        "30 END",
        "100 DEF PROCa",
        "110 X = FNf(4)",
        "120 PRINT X",
        "130 ENDPROC",
        "150 DEF FNf(N)",
        "160 = N + 1",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "5\nback\n");
}

#[test]
fn test_recursive_proc_in_fn_ends_correctly() {
    let (result, output) = run(&[
        "10 X = FNf(4)",
        "20 PRINT X",
        "30 END",
        "100 DEF FNf(N)",
        "110 T = 0",
        "120 PROCsum(N)",
        "130 = T",
        "200 DEF PROCsum(K)",
        "210 IF K = 0 THEN ENDPROC",
        "220 T = T + K",
        "230 PROCsum(K - 1)",
        "240 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "10\n");
}

#[test]
fn test_recursive_proc_ends_correctly() {
    let (result, output) = run(&[
        "10 PROCcount(3)",
        "20 PRINT \"end\"",
        "30 END",
        "100 DEF PROCcount(K)",
        "110 IF K = 0 THEN ENDPROC",
        "120 PRINT K",
        "130 PROCcount(K - 1)",
        "140 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "3\n2\n1\nend\n");
}

#[test]
fn test_endproc_with_no_proc_at_top_level_gives_no_proc() {
    let (result, output) = run(&["10 PRINT \"a\"", "20 ENDPROC"]);
    assert_eq!(result, Err("No PROC at line 20".to_string()));
    assert_eq!(output, "a\n");
}
