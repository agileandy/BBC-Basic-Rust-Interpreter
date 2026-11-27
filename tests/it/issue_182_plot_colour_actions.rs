//! Issue 182: the low 2 bits of a PLOT code select the colour. 1 draws in
//! the graphics foreground colour, 2 in the logical inverse colour and 3 in
//! the graphics background colour. PLOT 129 and 133 are the flood fill of
//! FILL. GCOL keeps one action for the foreground and one for the
//! background, and CLG uses the background colour and the background action.

use crate::common::run_with_limit;

#[test]
fn test_a_line_draws_in_the_foreground_the_background_and_the_inverse_colour() {
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : MOVE 100, 100 : PLOT 5, 200, 100 : PRINT POINT(150, 100)",
        "20 MOVE 100, 100 : PLOT 7, 200, 100 : PRINT POINT(150, 100)",
        "30 GCOL 0, 1 : MOVE 100, 100 : PLOT 5, 200, 100",
        "40 MOVE 100, 100 : PLOT 6, 200, 100 : PRINT POINT(150, 100)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         3\n         0\n         2\n");
}

#[test]
fn test_a_relative_line_uses_the_same_colours() {
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : MOVE 100, 100 : PLOT 1, 100, 0 : PRINT POINT(150, 100)",
        "20 MOVE 100, 100 : PLOT 3, 100, 0 : PRINT POINT(150, 100)",
        "30 GCOL 0, 1 : MOVE 100, 100 : PLOT 1, 100, 0",
        "40 MOVE 100, 100 : PLOT 2, 100, 0 : PRINT POINT(150, 100)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         3\n         0\n         2\n");
}

#[test]
fn test_a_point_draws_in_the_foreground_the_background_and_the_inverse_colour() {
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : PLOT 69, 100, 100 : PRINT POINT(100, 100)",
        "20 PLOT 71, 100, 100 : PRINT POINT(100, 100)",
        "30 GCOL 0, 1 : PLOT 69, 100, 100 : PLOT 70, 100, 100 : PRINT POINT(100, 100)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         3\n         0\n         2\n");
}

#[test]
fn test_a_triangle_draws_in_the_foreground_the_background_and_the_inverse_colour() {
    // (150, 120) is inside the triangle (100, 100), (200, 100), (150, 200)
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : MOVE 100, 100 : MOVE 200, 100 : PLOT 85, 150, 200",
        "20 PRINT POINT(150, 120)",
        "30 MOVE 100, 100 : MOVE 200, 100 : PLOT 87, 150, 200",
        "40 PRINT POINT(150, 120)",
        "50 GCOL 0, 1 : MOVE 100, 100 : MOVE 200, 100 : PLOT 85, 150, 200",
        "60 MOVE 100, 100 : MOVE 200, 100 : PLOT 86, 150, 200",
        "70 PRINT POINT(150, 120)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         3\n         0\n         2\n");
}

#[test]
fn test_a_circle_draws_in_the_foreground_the_background_and_the_inverse_colour() {
    // (348, 310) is a point on the circle of radius 50. The outline sets it
    // 1 time. It sets a point on an axis or a diagonal 2 times, so the
    // inverse colour of that point gives the old colour.
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : MOVE 300, 300 : PLOT 149, 350, 300 : PRINT POINT(348, 310)",
        "20 MOVE 300, 300 : PLOT 151, 350, 300 : PRINT POINT(348, 310)",
        "30 GCOL 0, 1 : MOVE 300, 300 : PLOT 149, 350, 300",
        "40 MOVE 300, 300 : PLOT 150, 350, 300 : PRINT POINT(348, 310)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         3\n         0\n         2\n");
}

#[test]
fn test_plot_7_uses_the_background_action() {
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : GCOL 0, 2 : PLOT 69, 100, 100",
        "20 GCOL 3, 129 : PLOT 71, 100, 100 : PRINT POINT(100, 100)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         3\n");
}

/// A box of colour 3 from (100, 100) to (200, 200), then a fill in colour 1
/// with `fill` from inside the box. The output gives a point inside, a point
/// on the box and a point outside.
fn run_box_fill(fill: &str) -> (Result<(), String>, String) {
    let fill_line = format!("20 GCOL 0, 1 : {fill}");
    run_with_limit(&[
        "10 MODE 1 : MOVE 100, 100 : DRAW 200, 100 : DRAW 200, 200 : DRAW 100, 200 : DRAW 100, 100",
        &fill_line,
        "30 PRINT POINT(150, 150); POINT(100, 150); POINT(50, 50)",
    ])
}

#[test]
fn test_plot_133_fills_the_same_pixels_as_fill() {
    let (result, output) = run_box_fill("FILL 150, 150");
    assert_eq!(result, Ok(()));
    assert_eq!(output, "         130\n");

    let (result, output) = run_box_fill("PLOT 133, 150, 150");
    assert_eq!(result, Ok(()));
    assert_eq!(output, "         130\n");
}

#[test]
fn test_plot_129_fills_from_a_point_relative_to_the_last_point() {
    let (result, output) = run_box_fill("MOVE 140, 140 : PLOT 129, 10, 10");

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         130\n");
}

#[test]
fn test_clg_uses_the_background_action() {
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : GCOL 0, 130 : CLG",
        "20 GCOL 3, 129 : CLG : PRINT POINT(10, 10)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         3\n");
}

#[test]
fn test_a_background_gcol_keeps_the_foreground_action() {
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : GCOL 0, 1 : PLOT 69, 100, 100",
        "20 GCOL 1, 2 : GCOL 0, 129 : PLOT 69, 100, 100 : PRINT POINT(100, 100)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         3\n");
}
