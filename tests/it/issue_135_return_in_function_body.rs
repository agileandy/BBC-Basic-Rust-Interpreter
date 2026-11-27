//! Tests that RETURN in a function body does not use a GOSUB of the caller
//! (issue #135).

use crate::common::run_with_limit as run;

// D1: RETURN with no GOSUB of the function body gives No GOSUB

#[test]
fn test_return_in_function_body_with_gosub_of_caller_gives_no_gosub() {
    let (result, output) = run(&[
        "10 GOSUB 100",
        "20 PRINT \"back at 20\"",
        "30 END",
        "100 X = FNf",
        "110 PRINT \"X=\";X",
        "120 RETURN",
        "200 DEF FNf",
        "210 RETURN",
        "220 = 1",
    ]);
    assert_eq!(result, Err("No GOSUB at line 210".to_string()));
    assert_eq!(output, "");
}

#[test]
fn test_return_in_function_body_is_err_38_and_later_gosub_works() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 300",
        "20 GOSUB 100",
        "30 END",
        "100 X = FNf",
        "110 PRINT \"not here\"",
        "120 RETURN",
        "200 DEF FNf",
        "210 RETURN",
        "220 = 1",
        "300 PRINT ERR; \" \"; ERL",
        "310 GOSUB 400",
        "320 PRINT \"after sub\"",
        "330 END",
        "400 PRINT \"sub\"",
        "410 RETURN",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "38 210\nsub\nafter sub\n");
}

// D2: a GOSUB and its RETURN in the function body still work

#[test]
fn test_gosub_and_return_in_function_body_work() {
    let (result, output) = run(&[
        "10 GOSUB 100",
        "20 PRINT \"back at 20\"",
        "30 END",
        "100 X = FNf",
        "110 PRINT \"X=\";X",
        "120 RETURN",
        "200 DEF FNf",
        "210 GOSUB 300",
        "220 = 1",
        "300 PRINT \"sub\"",
        "310 RETURN",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "sub\nX=1\nback at 20\n");
}

#[test]
fn test_proc_with_gosub_called_from_function_body_works() {
    let (result, output) = run(&[
        "10 GOSUB 100",
        "20 PRINT \"back at 20\"",
        "30 END",
        "100 X = FNf",
        "110 PRINT \"X=\";X",
        "120 RETURN",
        "200 DEF FNf",
        "210 PROCp",
        "220 = 2",
        "300 DEF PROCp",
        "310 GOSUB 400",
        "320 ENDPROC",
        "400 PRINT \"sub\"",
        "410 RETURN",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "sub\nX=2\nback at 20\n");
}

// D3: the main run has the floor 0

#[test]
fn test_return_in_main_run_with_no_gosub_gives_no_gosub() {
    let (result, output) = run(&["10 PRINT \"a\"", "20 RETURN"]);
    assert_eq!(result, Err("No GOSUB at line 20".to_string()));
    assert_eq!(output, "a\n");
}
