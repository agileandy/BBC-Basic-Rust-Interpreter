//! Issue #201: the statement `ERROR number, text$` makes an error with the
//! number and the text that the program gives, as in BBC BASIC for Windows.
//! ON ERROR traps it. `ERROR 0, text$` is a fatal error that ON ERROR does
//! not trap.

use crate::common::run_binary_read_first as run_binary;
use crate::common::run_with_limit as run;

#[test]
fn test_a_handler_gets_err_report_and_erl_of_the_error() {
    let (result, output) = run(&[
        "10 ON ERROR PRINT ;ERR;\" \";REPORT$;\" \";ERL : END",
        "20 ERROR 100, \"my error\"",
        "30 PRINT \"not here\"",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "100 my error 20\n");
}

#[test]
fn test_report_prints_the_text_of_the_error() {
    let (result, output) = run(&[
        "10 ON ERROR REPORT : PRINT : END",
        "20 ERROR 150, \"bad thing\"",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "bad thing\n");
}

#[test]
fn test_with_no_handler_the_program_stops_with_the_text() {
    let (result, output) = run(&[
        "10 PRINT \"a\"",
        "20 ERROR 100, \"my error\"",
        "30 PRINT \"b\"",
    ]);
    assert_eq!(result, Err("my error at line 20".to_string()));
    assert_eq!(output, "a\n");
}

#[test]
fn test_error_0_is_fatal_and_the_handler_does_not_trap_it() {
    let (result, output) = run(&[
        "10 ON ERROR PRINT \"trapped\" : END",
        "20 ERROR 0, \"fatal\"",
        "30 PRINT \"not here\"",
    ]);
    assert_eq!(result, Err("fatal at line 20".to_string()));
    assert_eq!(output, "");
}

#[test]
fn test_error_18_acts_as_the_real_division_by_zero() {
    let (result, output) = run(&[
        "10 ON ERROR PRINT ;ERR;\" \";REPORT$ : END",
        "20 ERROR 18, \"Division by zero\"",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "18 Division by zero\n");

    let (result, _) = run(&["10 ERROR 18, \"Division by zero\""]);
    assert_eq!(result, Err("Division by zero at line 10".to_string()));
}

#[test]
fn test_the_number_and_the_text_are_expressions() {
    let (result, output) = run(&[
        "10 ON ERROR PRINT ;ERR;\" \";REPORT$ : END",
        "20 n% = 50 : a$ = \"file \"",
        "30 ERROR n% * 2 + 1, a$ + STR$(7) + \" missing\"",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "101 file 7 missing\n");
}

#[test]
fn test_an_error_in_a_procedure_goes_to_the_handler_of_the_caller() {
    let (result, output) = run(&[
        "10 ON ERROR PRINT ;ERR;\" \";ERL : END",
        "20 PROCfail",
        "30 END",
        "40 DEF PROCfail",
        "50 ERROR 120, \"in proc\"",
        "60 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "120 50\n");
}

#[test]
fn test_the_handler_runs_outside_the_open_loop() {
    // The error clears the stacks as each trapped error does (issue #94)
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 FOR i% = 1 TO 3",
        "30 IF i% = 2 THEN ERROR 101, \"stop\"",
        "40 NEXT",
        "50 END",
        "100 ON ERROR OFF",
        "110 PRINT ;i%",
        "120 NEXT",
    ]);
    assert_eq!(result, Err("No FOR at line 120".to_string()));
    assert_eq!(output, "2\n");
}

#[test]
fn test_error_with_1_argument_is_a_syntax_error() {
    let (result, _) = run(&["10 ERROR 100"]);
    assert_eq!(
        result,
        Err("Syntax error at line 10: ERROR needs a number and a text: ERROR n, text$".to_string())
    );
}

#[test]
fn test_error_with_no_argument_is_a_syntax_error() {
    let (result, _) = run(&["10 ERROR"]);
    assert_eq!(
        result,
        Err("Syntax error at line 10: ERROR needs a number and a text: ERROR n, text$".to_string())
    );
}

#[test]
fn test_error_with_a_string_number_is_a_type_mismatch() {
    let (result, _) = run(&["10 ERROR \"a\", \"b\""]);
    assert_eq!(result, Err("Type mismatch at line 10".to_string()));
}

#[test]
fn test_error_with_a_number_text_is_a_type_mismatch() {
    let (result, _) = run(&["10 ERROR 100, 5"]);
    assert_eq!(result, Err("Type mismatch at line 10".to_string()));
}

#[test]
fn test_err_and_the_short_form_err_dot_are_unchanged() {
    // ERR. is the short form of ERROR (issue #167)
    let (result, output) = run(&["10 ON ERROR PRINT ;ERR : END", "20 ERR. 77, \"short\""]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "77\n");

    let (result, output) = run(&["10 PRINT ;ERR;\" \";ERL"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "0 0\n");
}

#[test]
fn test_names_that_start_with_error_still_work() {
    let (result, output) = run(&[
        "10 ERRORS% = 3",
        "20 DIM ERRORX(2)",
        "30 ERRORX(1) = 7",
        "40 PRINT ;ERRORS%;\" \";ERRORX(1)",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "3 7\n");
}

#[test]
fn test_a_number_out_of_0_to_255_keeps_its_low_byte() {
    // The manual does not give a rule. ERR is one byte, so the number keeps
    // its low byte: 356 is 100, and 256 is 0, which is fatal.
    let (result, output) = run(&["10 ON ERROR PRINT ;ERR : END", "20 ERROR 356, \"big\""]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "100\n");

    let (result, output) = run(&[
        "10 ON ERROR PRINT \"trapped\" : END",
        "20 ERROR 256, \"zero\"",
    ]);
    assert_eq!(result, Err("zero at line 20".to_string()));
    assert_eq!(output, "");
}

#[test]
fn test_the_binary_shows_the_text_of_an_untrapped_error() {
    let output = run_binary("10 ERROR 100, \"my error\"\nRUN\nEXIT\n");
    assert!(
        output.contains("my error at line 10"),
        "output was: {output}"
    );
}
