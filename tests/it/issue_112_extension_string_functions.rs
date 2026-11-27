//! Issue 112: UPPER$, LOWER$ and REPORT$. These are modern extensions, not
//! BBC BASIC. Old programs that use these names for variables or arrays still
//! work.

use crate::common::run_binary_read_first as run_binary;
use crate::common::run_with_limit as run;

#[test]
fn test_upper_and_lower_of_a_literal_in_print() {
    let (result, output) = run(&["10 PRINT UPPER$(\"ab\"); LOWER$(\"CD\")"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "ABcd\n");
}

#[test]
fn test_upper_and_lower_of_a_string_variable() {
    let (result, output) = run(&["10 A$ = \"Ab\"", "20 PRINT UPPER$(A$); LOWER$(A$)"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "ABab\n");
}

#[test]
fn test_upper_and_lower_in_assignment_comparison_and_argument() {
    let (result, output) = run(&[
        "10 B$ = LOWER$(\"XY\")",
        "20 PRINT B$",
        "30 IF UPPER$(\"x\") = \"X\" THEN PRINT \"YES\"",
        "40 PRINT LEN(UPPER$(\"ab\"))",
        "50 PRINT UPPER$(\"a\") + \"b\"",
        "60 PRINT LEFT$(LOWER$(\"ABC\"), 2)",
        "70 PRINT UPPER$(LOWER$(\"Q\") + \"r\")",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "xy\nYES\n2\nAb\nab\nQR\n");
}

#[test]
fn test_upper_with_two_arguments_gives_a_syntax_error() {
    let (result, _) = run(&["10 PRINT UPPER$(\"a\", \"b\")"]);

    assert_eq!(
        result,
        Err("Syntax error at line 10: UPPER$ requires 1 argument".to_string())
    );
}

#[test]
fn test_report_string_after_a_trapped_error_gives_the_text_of_report() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 X = 1 / 0",
        "30 END",
        "100 REPORT",
        "110 PRINT \"|\"; REPORT$",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "Division by zero|Division by zero\n");
}

#[test]
fn test_report_string_with_no_error_is_empty() {
    let (result, output) = run(&["10 PRINT \"[\"; REPORT$; \"]\"; LEN(REPORT$)"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "[]0\n");
}

#[test]
fn test_a_variable_named_report_string_keeps_its_meaning() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 REPORT$ = \"mine\"",
        "30 X = 1 / 0",
        "40 END",
        "100 PRINT REPORT$",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "mine\n");
}

#[test]
fn test_variables_named_upper_and_lower_with_no_bracket_still_work() {
    let (result, output) = run(&[
        "10 UPPER$ = \"u\"",
        "20 LOWER$ = \"l\"",
        "30 PRINT UPPER$; LOWER$",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "ul\n");
}

#[test]
fn test_arrays_named_upper_and_lower_keep_their_meaning() {
    let (result, output) = run(&[
        "10 DIM LOWER$(3), UPPER$(2)",
        "20 LOWER$(1) = \"q\"",
        "30 UPPER$(2) = \"w\"",
        "40 PRINT LOWER$(1); UPPER$(2); LEN(LOWER$(3))",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "qw0\n");
}

#[test]
fn test_typed_lines_call_the_three_functions() {
    let output = run_binary(
        "PRINT UPPER$(\"ab\")\nA$ = \"Ef\" : PRINT LOWER$(A$)\nPRINT 1 / 0\nPRINT REPORT$\nEXIT\n",
    );

    assert!(output.contains("> AB\n"), "output: {output}");
    assert!(output.contains("> ef\n"), "output: {output}");
    assert!(output.contains("> Division by zero\n"), "output: {output}");
}

#[test]
fn test_a_typed_program_calls_the_three_functions() {
    let output = run_binary(
        "10 ON ERROR GOTO 40\n20 PRINT UPPER$(\"ab\"); LOWER$(\"CD\")\n30 X = 1 / 0\n40 PRINT REPORT$\nRUN\nEXIT\n",
    );

    assert!(
        output.contains("ABcd\nDivision by zero\n"),
        "output: {output}"
    );
}
