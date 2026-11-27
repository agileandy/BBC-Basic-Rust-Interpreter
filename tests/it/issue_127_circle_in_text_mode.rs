//! Issue #127: CIRCLE and ELLIPSE draw nothing in a screen mode with no
//! graphics (modes 3, 6 and 7), as PLOT, MOVE and DRAW do. RECTANGLE and
//! FILL also drew in mode 7, so they get the same check (plan item D2). In a
//! graphics mode each one draws as before.

use crate::common::run_with_limit;

/// Run the program and give the output, after a check that it ends with no error.
fn output_of(source: &[&str]) -> String {
    let (result, output) = run_with_limit(source);
    assert_eq!(result, Ok(()));
    output
}

/// A program that selects the mode, draws a circle at (100,100) with radius
/// 50, and prints the state of a pixel on the circle.
fn circle_program(mode: &str) -> Vec<String> {
    vec![
        format!("10 MODE {mode}"),
        "20 CIRCLE 100,100,50".to_string(),
        "30 PRINT POINT(150,100)".to_string(),
    ]
}

/// A program that selects the mode, draws an ellipse at (100,100) with
/// semi-axes 60 and 30, and prints the state of a pixel on the ellipse.
fn ellipse_program(mode: &str) -> Vec<String> {
    vec![
        format!("10 MODE {mode}"),
        "20 ELLIPSE 100,100,60,30".to_string(),
        "30 PRINT POINT(160,100)".to_string(),
    ]
}

fn run_program_lines(lines: &[String]) -> String {
    let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
    output_of(&lines)
}

#[test]
fn test_circle_in_mode_1_draws() {
    assert_eq!(run_program_lines(&circle_program("1")), "-1\n");
}

#[test]
fn test_circle_in_mode_3_sets_no_pixel() {
    assert_eq!(run_program_lines(&circle_program("3")), "0\n");
}

#[test]
fn test_circle_in_mode_6_sets_no_pixel() {
    assert_eq!(run_program_lines(&circle_program("6")), "0\n");
}

#[test]
fn test_circle_in_mode_7_sets_no_pixel() {
    assert_eq!(run_program_lines(&circle_program("7")), "0\n");
}

#[test]
fn test_ellipse_in_mode_1_draws() {
    assert_eq!(run_program_lines(&ellipse_program("1")), "-1\n");
}

#[test]
fn test_ellipse_in_mode_3_sets_no_pixel() {
    assert_eq!(run_program_lines(&ellipse_program("3")), "0\n");
}

#[test]
fn test_ellipse_in_mode_6_sets_no_pixel() {
    assert_eq!(run_program_lines(&ellipse_program("6")), "0\n");
}

#[test]
fn test_ellipse_in_mode_7_sets_no_pixel() {
    assert_eq!(run_program_lines(&ellipse_program("7")), "0\n");
}

#[test]
fn test_circle_and_ellipse_in_mode_7_give_no_error() {
    let output = output_of(&[
        "10 MODE 7",
        "20 CIRCLE 100,100,50",
        "30 ELLIPSE 100,100,60,30",
        "40 PRINT \"done\"",
    ]);

    assert_eq!(output, "done\n");
}

#[test]
fn test_circle_in_mode_1_after_mode_7_draws() {
    let output = output_of(&[
        "10 MODE 7",
        "20 CIRCLE 100,100,50",
        "30 MODE 1",
        "40 CIRCLE 100,100,50",
        "50 PRINT POINT(150,100)",
    ]);

    assert_eq!(output, "-1\n");
}

#[test]
fn test_rectangle_in_mode_7_sets_no_pixel() {
    let output = output_of(&[
        "10 MODE 7",
        "20 RECTANGLE FILL 100,100,50,50",
        "30 PRINT POINT(120,120)",
    ]);

    assert_eq!(output, "0\n");
}

#[test]
fn test_rectangle_in_mode_1_draws() {
    let output = output_of(&[
        "10 MODE 1",
        "20 RECTANGLE FILL 100,100,50,50",
        "30 PRINT POINT(120,120)",
    ]);

    assert_eq!(output, "-1\n");
}

#[test]
fn test_fill_in_mode_1_draws() {
    let output = output_of(&["10 MODE 1", "20 FILL 100,100", "30 PRINT POINT(100,100)"]);

    assert_eq!(output, "-1\n");
}

#[test]
fn test_fill_in_mode_7_sets_no_pixel() {
    let output = output_of(&["10 MODE 7", "20 FILL 100,100", "30 PRINT POINT(100,100)"]);

    assert_eq!(output, "0\n");
}
