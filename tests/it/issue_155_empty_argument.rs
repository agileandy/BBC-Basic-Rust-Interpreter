//! Issue 155: an empty value in a list of values (2 commas together, or a
//! comma at the start or at the end) gives a syntax error. PRINT, INPUT and
//! VDU have their own rules and do not change.

use crate::common::run_lines;

/// The error text for an empty value in a list of values.
fn empty_value_error(line: u16) -> Result<(), String> {
    Err(format!(
        "Syntax error at line {}: Missing value at a comma",
        line
    ))
}

/// Remove the screen clear that MODE writes before the output.
fn without_clear(output: &str) -> &str {
    output.strip_prefix("\x1b[2J\x1b[H").unwrap_or(output)
}

#[test]
fn test_move_with_2_commas_together_gives_a_syntax_error() {
    let (result, output) = run_lines(&["10 MODE 1 : MOVE 100,,200 : PRINT \"ran\""]);

    assert_eq!(result, empty_value_error(10));
    assert!(!output.contains("ran"));
}

#[test]
fn test_move_with_a_comma_at_the_end_gives_a_syntax_error() {
    let (result, _) = run_lines(&["10 MODE 1 : MOVE 100,"]);

    assert_eq!(result, empty_value_error(10));
}

#[test]
fn test_plot_with_commas_at_the_end_gives_a_syntax_error() {
    let (result, _) = run_lines(&["10 MODE 1 : PLOT 69,,"]);

    assert_eq!(result, empty_value_error(10));
}

#[test]
fn test_gcol_with_a_comma_at_the_start_gives_a_syntax_error() {
    let (result, _) = run_lines(&["10 MODE 1 : GCOL ,1"]);

    assert_eq!(result, empty_value_error(10));
}

#[test]
fn test_an_array_index_with_2_commas_together_gives_a_syntax_error() {
    let (result, _) = run_lines(&["10 DIM A(2,2)", "20 A(1,,2) = 5"]);

    assert_eq!(result, empty_value_error(20));
}

#[test]
fn test_move_with_2_values_is_unchanged() {
    let (result, output) = run_lines(&[
        "10 MODE 1 : MOVE 100,200 : DRAW 300,200",
        "20 PRINT POINT(200,200)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(without_clear(&output), "-1\n");
}

#[test]
fn test_move_with_no_value_keeps_its_error() {
    let (result, _) = run_lines(&["10 MOVE"]);

    assert_eq!(
        result,
        Err("Syntax error at line 10: MOVE requires x, y parameters".to_string())
    );
}

#[test]
fn test_print_with_2_commas_together_is_unchanged() {
    let (result, output) = run_lines(&["10 PRINT 1,,2"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1                   2\n");
}

#[test]
fn test_vdu_with_semicolons_is_unchanged() {
    let (result, output) = run_lines(&["10 VDU 23,1,0;0;0;0;", "20 PRINT \"ran\""]);

    assert_eq!(result, Ok(()));
    assert!(output.ends_with("ran\n"));
}
