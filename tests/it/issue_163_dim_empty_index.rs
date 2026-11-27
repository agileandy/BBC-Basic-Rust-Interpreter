//! Issue 163: an empty size in DIM (2 commas together, or a comma at the
//! start or at the end of the sizes) gives a syntax error. An empty item in
//! the list of DIM items gives the same error.

use crate::common::run_lines;

/// The error text for an empty value in a list of values (issue #155).
fn empty_value_error(line: u16) -> Result<(), String> {
    Err(format!(
        "Syntax error at line {}: Missing value at a comma",
        line
    ))
}

#[test]
fn test_dim_with_2_commas_together_in_the_sizes_gives_a_syntax_error() {
    let (result, output) = run_lines(&["10 DIM B(1,,2) : PRINT \"ran\""]);

    assert_eq!(result, empty_value_error(10));
    assert!(!output.contains("ran"));
}

#[test]
fn test_dim_with_a_comma_at_the_end_of_the_sizes_gives_a_syntax_error() {
    let (result, _) = run_lines(&["10 DIM A(1,)"]);

    assert_eq!(result, empty_value_error(10));
}

#[test]
fn test_dim_with_a_comma_at_the_start_of_the_sizes_gives_a_syntax_error() {
    let (result, _) = run_lines(&["10 DIM A(,1)"]);

    assert_eq!(result, empty_value_error(10));
}

#[test]
fn test_dim_with_an_empty_item_between_arrays_gives_a_syntax_error() {
    let (result, _) = run_lines(&["10 DIM A(1), , B(2)"]);

    assert_eq!(result, empty_value_error(10));
}

#[test]
fn test_dim_with_a_comma_at_the_end_of_the_items_gives_a_syntax_error() {
    let (result, _) = run_lines(&["10 DIM A(1),"]);

    assert_eq!(result, empty_value_error(10));
}

#[test]
fn test_dim_block_with_a_comma_at_the_end_gives_a_syntax_error() {
    let (result, _) = run_lines(&["10 DIM P% 10,"]);

    assert_eq!(result, empty_value_error(10));
}

#[test]
fn test_dim_block_with_an_empty_item_after_it_gives_a_syntax_error() {
    let (result, _) = run_lines(&["10 DIM P% 10, , B(2)"]);

    assert_eq!(result, empty_value_error(10));
}

#[test]
fn test_dim_of_2_arrays_is_unchanged() {
    let (result, output) = run_lines(&[
        "10 DIM A(1, 2), B$(3)",
        "20 A(1, 2) = 7 : B$(3) = \"x\"",
        "30 PRINT A(1, 2); B$(3)",
    ]);

    assert_eq!(result, Ok(()));
    assert!(output.contains("7x"), "output was {:?}", output);
}

#[test]
fn test_dim_size_with_a_comma_in_brackets_is_unchanged() {
    let (result, output) = run_lines(&["10 DIM A(LEN(MID$(\"abc\", 2)), 1)", "20 PRINT \"ok\""]);

    assert_eq!(result, Ok(()));
    assert!(output.contains("ok"), "output was {:?}", output);
}

#[test]
fn test_dim_block_is_unchanged() {
    let (result, output) = run_lines(&["10 DIM P% 10", "20 ?P% = 65 : PRINT ?P%"]);

    assert_eq!(result, Ok(()));
    assert!(output.contains("65"), "output was {:?}", output);
}
