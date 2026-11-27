//! Issue #153: READ puts a DATA value into an array element, as it puts a
//! value into a variable.

use crate::common::run_with_limit as run;

#[test]
fn test_read_in_a_loop_fills_an_array() {
    let (result, output) = run(&[
        "10 DIM A(3)",
        "20 FOR I = 1 TO 3 : READ A(I) : NEXT",
        "30 PRINT A(1); \" \"; A(2); \" \"; A(3)",
        "40 DATA 10, 20.5, 30",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "        10 20.5 30\n");
}

#[test]
fn test_read_into_string_and_integer_arrays() {
    let (result, output) = run(&[
        "10 DIM N$(2), C%(2)",
        "20 READ N$(1), C%(2), N$(2)",
        "30 PRINT N$(1); \" \"; C%(2); \" \"; N$(2)",
        "40 DATA abc, 7.9, \"x, y\"",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "abc 7 x, y\n");
}

#[test]
fn test_read_into_a_2_dimension_element() {
    let (result, output) = run(&[
        "10 DIM Q(3,3), N$(2)",
        "20 READ Q(1,2), N$(1)",
        "30 PRINT Q(1,2); \" \"; N$(1)",
        "40 DATA 5, abc",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "         5 abc\n");
}

#[test]
fn test_index_uses_the_value_read_before_it() {
    let (result, output) = run(&[
        "10 DIM A(5)",
        "20 READ I, A(I)",
        "30 PRINT I; \" \"; A(3)",
        "40 DATA 3, 99",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "         3 99\n");
}

#[test]
fn test_index_expression_with_a_function_call() {
    let (result, output) = run(&[
        "10 DIM A(5,5)",
        "20 READ A(ABS(-2), 1 + 1)",
        "30 PRINT A(2,2)",
        "40 DATA 8",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "         8\n");
}

#[test]
fn test_index_out_of_range_gives_subscript() {
    let (result, _) = run(&["10 DIM A(3)", "20 READ A(4)", "30 DATA 1"]);
    assert_eq!(result, Err("Subscript at line 20".to_string()));
}

#[test]
fn test_out_of_data_into_an_element() {
    let (result, _) = run(&["10 DIM A(3)", "20 READ A(1), A(2)", "30 DATA 1"]);
    assert_eq!(result, Err("Out of DATA at line 20".to_string()));
}

#[test]
fn test_read_variables_is_unchanged() {
    let (result, output) = run(&[
        "10 READ A, B$, C%",
        "20 PRINT A; \" \"; B$; \" \"; C%",
        "30 DATA 1.5, hello, 4",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "       1.5 hello 4\n");
}

#[test]
fn test_read_a_number_is_a_syntax_error() {
    let (result, _) = run(&["10 READ 5", "20 DATA 1"]);
    assert_eq!(
        result,
        Err("Syntax error at line 10: Expected variable name in READ".to_string())
    );
}

#[test]
fn test_read_an_expression_is_a_syntax_error() {
    let (result, _) = run(&["10 READ A + 1", "20 DATA 1"]);
    assert_eq!(
        result,
        Err("Syntax error at line 10: Expected variable name in READ".to_string())
    );
}

#[test]
fn test_read_tokens_after_an_element_is_a_syntax_error() {
    let (result, _) = run(&["10 DIM A(3)", "20 READ A(1) B", "30 DATA 1"]);
    assert_eq!(
        result,
        Err("Syntax error at line 20: Expected variable name in READ".to_string())
    );
}

/// An empty target between commas is a syntax error, as an empty value is in
/// a list of expressions (issue #155).
#[test]
fn test_read_with_an_empty_target_is_a_syntax_error() {
    for line in ["10 READ A,,B", "10 READ A,", "10 READ ,A", "10 READ A(1,)"] {
        let (result, _) = run(&["5 DIM A(3)", line, "20 DATA 1, 2"]);
        assert_eq!(
            result,
            Err("Syntax error at line 10: Missing value at a comma".to_string()),
            "{}",
            line
        );
    }
}

#[test]
fn test_read_with_no_target_keeps_its_error() {
    let (result, _) = run(&["10 READ", "20 DATA 1"]);
    assert_eq!(
        result,
        Err("Syntax error at line 10: READ requires at least one variable".to_string())
    );
}
