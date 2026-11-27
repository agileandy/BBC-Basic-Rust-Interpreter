//! Issue #144: a numeric evaluator that gets a string value gives Type
//! mismatch (ERR 6), as on a BBC Micro. A name that is no function at all
//! keeps its error.

use crate::common::run_with_limit as run;

fn type_mismatch_at_line_10() -> Result<(), String> {
    Err("Type mismatch at line 10".to_string())
}

#[test]
fn test_real_variable_gets_left_string_function() {
    let (result, _) = run(&["10 A = LEFT$(\"a\", 1)", "20 END"]);

    assert_eq!(result, type_mismatch_at_line_10());
}

#[test]
fn test_integer_variable_gets_chr_string_function() {
    let (result, _) = run(&["10 A% = CHR$(65)", "20 END"]);

    assert_eq!(result, type_mismatch_at_line_10());
}

#[test]
fn test_real_variable_gets_string_literal() {
    let (result, _) = run(&["10 A = \"x\"", "20 END"]);

    assert_eq!(result, type_mismatch_at_line_10());
}

#[test]
fn test_real_variable_gets_string_variable() {
    let (result, _) = run(&["10 B$ = \"q\" : A = B$", "20 END"]);

    assert_eq!(result, type_mismatch_at_line_10());
}

#[test]
fn test_string_function_in_a_sum_gives_type_mismatch() {
    let (result, _) = run(&["10 A = 1 + LEFT$(\"a\", 1)", "20 END"]);

    assert_eq!(result, type_mismatch_at_line_10());
}

#[test]
fn test_hex_string_function_gives_type_mismatch() {
    let (result, _) = run(&["10 A = STR$~(5)", "20 END"]);

    assert_eq!(result, type_mismatch_at_line_10());
}

#[test]
fn test_error_handler_gets_err_6_for_string_function() {
    let (result, output) = run(&[
        "10 ON ERROR PRINT ERR : END",
        "20 A = LEFT$(\"a\", 1)",
        "30 END",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "6\n");
}

#[test]
fn test_error_handler_gets_err_6_for_each_issue_case() {
    for case in [
        "30 A = LEFT$(\"a\", 1)",
        "30 A% = CHR$(65)",
        "30 A = \"x\"",
        "30 A = B$",
    ] {
        let (result, output) = run(&[
            "10 B$ = \"q\"",
            "20 ON ERROR PRINT ERR : END",
            case,
            "40 END",
        ]);

        assert_eq!(result, Ok(()), "{case}");
        assert_eq!(output, "6\n", "{case}");
    }
}

#[test]
fn test_name_that_is_no_function_keeps_its_error() {
    let (result, _) = run(&["10 A = NOSUCH(1)", "20 END"]);

    assert_eq!(result, Err("Array at line 10".to_string()));
}

#[test]
fn test_numeric_function_in_numeric_context_still_works() {
    let (result, output) = run(&["10 A = LEN(\"abc\") + ASC(\"A\") : PRINT A", "20 END"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "68\n");
}

#[test]
fn test_string_variable_gets_number_gives_type_mismatch() {
    let (result, _) = run(&["10 A$ = 5", "20 END"]);

    assert_eq!(result, type_mismatch_at_line_10());
}

/// Issue #144 did not change this error. Issue #146 made it Type mismatch.
#[test]
fn test_string_variable_gets_numeric_function_gives_type_mismatch() {
    let (result, _) = run(&["10 A$ = SIN(1)", "20 END"]);

    assert_eq!(result, type_mismatch_at_line_10());
}
