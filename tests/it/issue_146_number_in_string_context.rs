//! Issue #146: a string evaluator that gets a numeric value gives Type
//! mismatch (ERR 6), as on a BBC Micro. A user FN with a string result still
//! works. A name that is no function, and a statement keyword in an
//! expression, keep their errors.

use crate::common::run_with_limit as run;

/// The numeric values in a string context that the issue names, and two more:
/// a function with no brackets and a numeric variable.
const NUMBER_IN_STRING_CASES: &[&str] = &[
    "A$ = SIN(1)",
    "A$ = LEN(\"ab\")",
    "A$ = TRUE",
    "A$ = PI",
    "PRINT LEN(\"ab\") + \"x\"",
    "A$ = SIN 1",
    "X = 1 : A$ = X",
];

#[test]
fn test_numeric_value_in_string_context_gives_type_mismatch() {
    for case in NUMBER_IN_STRING_CASES {
        let line = format!("10 {case}");
        let (result, _) = run(&[line.as_str(), "20 END"]);

        assert_eq!(
            result,
            Err("Type mismatch at line 10".to_string()),
            "{case}"
        );
    }
}

#[test]
fn test_error_handler_gets_err_6_for_numeric_value_in_string_context() {
    for case in NUMBER_IN_STRING_CASES {
        let line = format!("20 {case}");
        let (result, output) = run(&["10 ON ERROR PRINT ERR : END", line.as_str(), "30 END"]);

        assert_eq!(result, Ok(()), "{case}");
        assert_eq!(output, "6\n", "{case}");
    }
}

#[test]
fn test_single_line_function_with_string_result_still_works() {
    let (result, output) = run(&[
        "10 A$ = FNt : PRINT A$",
        "20 END",
        "30 DEF FNt = \"one line\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "one line\n");
}

#[test]
fn test_multi_line_function_with_string_result_still_works() {
    let (result, output) = run(&[
        "10 A$ = FNt : PRINT A$",
        "20 END",
        "30 DEF FNt",
        "40 = \"many lines\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "many lines\n");
}

#[test]
fn test_name_that_is_no_function_keeps_its_error() {
    let (result, _) = run(&["10 A$ = NOSUCH(1)", "20 END"]);

    assert_eq!(result, Err("Array at line 10".to_string()));
}

#[test]
fn test_statement_keyword_in_expression_keeps_its_syntax_error() {
    let (result, _) = run(&["10 A$ = PRINT(1)", "20 END"]);

    assert_eq!(
        result,
        Err("Syntax error at line 10: Unknown string function: PRINT".to_string())
    );
}

#[test]
fn test_numeric_variable_with_no_value_keeps_its_error() {
    let (result, _) = run(&["10 A$ = X", "20 END"]);

    assert_eq!(result, Err("No such variable: X at line 10".to_string()));
}

#[test]
fn test_error_in_numeric_function_argument_comes_first() {
    let (result, _) = run(&["10 A$ = SQR(-1)", "20 END"]);

    assert_eq!(result, Err("-ve root at line 10".to_string()));
}

#[test]
fn test_string_functions_in_string_context_still_work() {
    let (result, output) = run(&[
        "10 A$ = STR$(LEN(\"ab\")) + CHR$(65) + STR$~(255) : PRINT A$",
        "20 END",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "2AFF\n");
}
