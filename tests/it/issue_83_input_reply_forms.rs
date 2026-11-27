//! Issue 83: INPUT into an array element, a reply in quotes, and the spaces
//! of a reply.

use crate::common::run_with_replies;

#[test]
fn test_input_stores_into_numeric_array_element() {
    let (result, output) = run_with_replies(
        &["10 DIM A(3)", "20 INPUT A(1)", "30 PRINT A(1) * 2"],
        &["21"],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "?\n        42\n");
}

#[test]
fn test_input_stores_into_string_array_element_with_variable_index() {
    let (result, output) = run_with_replies(
        &[
            "10 DIM A$(3)",
            "20 I% = 2",
            "30 INPUT \"Name\";A$(I%)",
            "40 PRINT \"[\";A$(2);\"]\"",
        ],
        &["Ann"],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "Name?\n[Ann]\n");
}

#[test]
fn test_input_stores_into_two_dimension_integer_element() {
    let (result, output) = run_with_replies(
        &["10 DIM M%(2, 2)", "20 INPUT M%(1, 2)", "30 PRINT M%(1, 2)"],
        &["7"],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "?\n         7\n");
}

#[test]
fn test_input_into_element_out_of_range_gives_subscript_error() {
    let (result, _) = run_with_replies(&["10 DIM A(3)", "20 INPUT A(4)"], &["1"]);

    let error = result.expect_err("index 4 is out of range");
    assert!(error.contains("Subscript"), "error was {error}");
}

#[test]
fn test_quoted_reply_keeps_its_comma() {
    let (result, output) = run_with_replies(
        &["10 INPUT A$, B$", "20 PRINT \"[\";A$;\"][\";B$;\"]\""],
        &["\"a,b\",c"],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "?\n[a,b][c]\n");
}

#[test]
fn test_quoted_reply_keeps_its_spaces() {
    let (result, output) = run_with_replies(
        &["10 INPUT A$", "20 PRINT \"[\";A$;\"]\""],
        &["  \"  x  \"  "],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "?\n[  x  ]\n");
}

#[test]
fn test_quoted_reply_into_array_elements() {
    let (result, output) = run_with_replies(
        &[
            "10 DIM N$(2)",
            "20 INPUT N$(1), N$(2)",
            "30 PRINT N$(1);\"/\";N$(2)",
        ],
        &["\"Smith, J\", \"Jones, K\""],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "?\nSmith, J/Jones, K\n");
}

#[test]
fn test_plain_input_keeps_trailing_spaces_of_each_value() {
    let (result, output) = run_with_replies(
        &["10 INPUT A$, B$", "20 PRINT \"[\";A$;\"][\";B$;\"]\""],
        &["  a  ,  b  "],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "?\n[a  ][b  ]\n");
}

#[test]
fn test_plain_input_removes_the_line_end_of_a_reply() {
    let (result, output) =
        run_with_replies(&["10 INPUT A$", "20 PRINT \"[\";A$;\"]\""], &["x \r\n"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "?\n[x ]\n");
}

#[test]
fn test_numeric_value_with_spaces_is_not_affected() {
    let (result, output) =
        run_with_replies(&["10 INPUT A, B%", "20 PRINT A + B%"], &["  1.5  ,  2  "]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "?\n       3.5\n");
}

#[test]
fn test_input_line_keeps_quotes_and_commas() {
    let (result, output) = run_with_replies(
        &["10 INPUT LINE A$", "20 PRINT \"[\";A$;\"]\""],
        &[" \"a,b\", c "],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "?\n[ \"a,b\", c ]\n");
}

#[test]
fn test_input_line_stores_into_array_element() {
    let (result, output) = run_with_replies(
        &[
            "10 DIM L$(2)",
            "20 INPUT LINE L$(2)",
            "30 PRINT \"[\";L$(2);\"]\"",
        ],
        &[" a, b "],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "?\n[ a, b ]\n");
}
