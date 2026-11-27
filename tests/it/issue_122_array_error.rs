//! Issue #122: a read or an assignment of an element of an array that has
//! no DIM gives the BBC BASIC error "Array", with ERR 14.

use crate::common::run_lines;

#[test]
fn test_read_of_real_array_with_no_dim_gives_array() {
    let (result, _) = run_lines(&["10 PRINT L(1)"]);

    assert_eq!(result, Err("Array at line 10".to_string()));
}

#[test]
fn test_assignment_to_array_with_no_dim_gives_array() {
    let (result, _) = run_lines(&["10 L(1) = 2"]);

    assert_eq!(result, Err("Array at line 10".to_string()));
}

#[test]
fn test_read_of_string_array_with_no_dim_gives_array() {
    let (result, _) = run_lines(&["10 PRINT L$(1)"]);

    assert_eq!(result, Err("Array at line 10".to_string()));
}

#[test]
fn test_read_of_integer_array_with_no_dim_gives_array() {
    let (result, _) = run_lines(&["10 PRINT L%(1)"]);

    assert_eq!(result, Err("Array at line 10".to_string()));
}

#[test]
fn test_array_error_has_err_14_in_a_handler() {
    let (result, output) = run_lines(&[
        "10 ON ERROR GOTO 100",
        "20 PRINT L(1)",
        "30 END",
        "100 PRINT ERR",
        "110 END",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "14\n");
}

#[test]
fn test_local_array_with_no_dim_gives_array() {
    let (result, _) = run_lines(&[
        "10 PROCREAD",
        "20 END",
        "100 DEF PROCREAD",
        "110 LOCAL A%()",
        "120 PRINT A%(0)",
        "130 ENDPROC",
    ]);

    assert_eq!(result, Err("Array at line 120".to_string()));
}

#[test]
fn test_scalar_that_does_not_exist_keeps_no_such_variable() {
    let (result, _) = run_lines(&["10 PRINT Q"]);

    assert_eq!(result, Err("No such variable: Q at line 10".to_string()));
}

#[test]
fn test_upper_function_keeps_its_path() {
    let (result, output) = run_lines(&["10 PRINT UPPER$(\"a\")"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "A\n");
}
