//! Issue 196: a token after a PRINT item that is not a separator and does not
//! start a new item gives a syntax error.
//!
//! Before this change, `PRINT A(1) #` printed the value and ignored the `#`.

use crate::common::run_lines as run;
use bbc_basic_interpreter::tokenizer::tokenize;

/// Run the program and check that it gives the error and prints nothing.
fn check_error(source: &[&str], message: &str) {
    let (result, output) = run(source);
    assert_eq!(result, Err(message.to_string()), "program: {:?}", source);
    assert_eq!(output, "", "program: {:?}", source);
}

/// Run the program and check that it ends with no error and gives the output.
fn check_output(source: &[&str], expected: &str) {
    let (result, output) = run(source);
    assert_eq!(result, Ok(()), "program: {:?}", source);
    assert_eq!(output, expected, "program: {:?}", source);
}

#[test]
fn test_hash_after_array_element() {
    check_error(
        &["10 DIM A(1)", "20 PRINT A(1) #", "30 PRINT \"RAN\""],
        "Syntax error at line 20: Unexpected # in PRINT",
    );
}

#[test]
fn test_dot_after_array_element() {
    check_error(
        &["10 DIM A(1)", "20 PRINT A(1).", "30 PRINT \"RAN\""],
        "Syntax error at line 20: Unexpected . in PRINT",
    );
}

#[test]
fn test_close_bracket_after_number() {
    check_error(
        &["10 PRINT 1 )", "20 PRINT \"RAN\""],
        "Syntax error at line 10: Unexpected ) in PRINT",
    );
}

/// The tokenizer has no `]` token, so it stops the line before PRINT reads it
#[test]
fn test_square_bracket_after_string() {
    let error = tokenize("10 PRINT \"a\" ]").expect_err("line gives an error");
    assert_eq!(
        error.to_string(),
        "Syntax error at line 10: Unknown character ']'"
    );
}

#[test]
fn test_stray_token_before_separator() {
    check_error(
        &["10 PRINT ;1 # ;2", "20 PRINT \"RAN\""],
        "Syntax error at line 10: Unexpected # in PRINT",
    );
}

#[test]
fn test_stray_token_in_print_file() {
    check_error(
        &["10 PRINT#0, 1 #", "20 PRINT \"RAN\""],
        "Syntax error at line 10: Unexpected # in PRINT",
    );
}

#[test]
fn test_two_numbers_with_no_separator() {
    check_output(&["10 PRINT ;1 2"], "12\n");
}

#[test]
fn test_string_then_variable_with_no_separator() {
    check_output(&["10 A$ = \"b\"", "20 PRINT \"a\"A$"], "ab\n");
}

#[test]
fn test_hex_then_number_with_no_separator() {
    check_output(&["10 PRINT ~10 58"], "         A        3A\n");
}

#[test]
fn test_semicolon_at_end() {
    check_output(
        &["10 A$ = \"b\"", "20 PRINT A$;", "30 PRINT ;\"c\""],
        "bc\n",
    );
}

#[test]
fn test_comma_only() {
    check_output(&["10 PRINT ,", "20 PRINT ;\"c\""], "\nc\n");
}

#[test]
fn test_tab_then_new_line_item() {
    check_output(&["10 PRINT TAB(3)\"x\"'\"y\""], "   x\ny\n");
}
