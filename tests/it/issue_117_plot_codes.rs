//! Issue 117: PLOT codes with low bits 0 and 4 only move, codes 128 to 191
//! do the action that BBC BASIC gives them, and CIRCLE records its points.

use crate::common::run_lines as run;

#[test]
fn test_plot_68_moves_and_sets_no_pixel() {
    let (result, output) = run(&[
        "10 MOVE 100, 100",
        "20 PLOT 68, 200, 200",
        "30 PRINT POINT(200, 200)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         0\n");
}

#[test]
fn test_plot_68_moves_the_last_point() {
    // The line from the last point (200, 200) goes to (210, 200).
    let (result, output) = run(&[
        "10 MOVE 100, 100",
        "20 PLOT 68, 200, 200",
        "30 PLOT 1, 10, 0",
        "40 PRINT POINT(205, 200); POINT(105, 100)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         10\n");
}

#[test]
fn test_plot_64_moves_relative_and_sets_no_pixel() {
    let (result, output) = run(&[
        "10 MOVE 100, 100",
        "20 PLOT 64, 10, 10",
        "30 PRINT POINT(110, 110)",
        "40 PLOT 1, 10, 0",
        "50 PRINT POINT(115, 110)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         0\n         1\n");
}

#[test]
fn test_plot_133_draws_no_triangle() {
    // Before the change, 2 PLOT 133 codes filled the triangle
    // (400, 400), (600, 400), (600, 600). The point (580, 420) is inside it.
    // Since issue 182, PLOT 133 is the flood fill of FILL, so the empty
    // canvas fills: (100, 100), outside the triangle, also has colour 1.
    let (result, output) = run(&[
        "10 MOVE 400, 400",
        "20 PLOT 133, 600, 600",
        "30 MOVE 600, 400",
        "40 PLOT 133, 600, 600",
        "50 PRINT POINT(580, 420); POINT(100, 100)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         11\n");
}

#[test]
fn test_plot_133_moves_the_last_point() {
    let (result, output) = run(&[
        "10 MOVE 100, 100",
        "20 PLOT 133, 200, 200",
        "30 PLOT 1, 10, 0",
        "40 PRINT POINT(205, 200)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         1\n");
}

#[test]
fn test_unsupported_codes_from_160_to_191_draw_nothing() {
    let (result, output) = run(&[
        "10 MOVE 300, 300",
        "20 PLOT 165, 400, 300",
        "30 PLOT 173, 300, 400",
        "40 PLOT 181, 300, 300",
        "50 PLOT 189, 400, 400",
        "60 PRINT POINT(350, 300); POINT(300, 350); POINT(330, 330)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         000\n");
}

#[test]
fn test_plot_145_draws_a_circle_outline_round_the_last_point() {
    // The centre is the last point and the relative point is on the circle.
    let (result, output) = run(&[
        "10 MOVE 300, 300",
        "20 PLOT 145, 50, 0",
        "30 PRINT POINT(350, 300); POINT(250, 300); POINT(300, 350); POINT(300, 300)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         1110\n");
}

#[test]
fn test_plot_149_draws_a_circle_through_an_absolute_point() {
    let (result, output) = run(&[
        "10 MOVE 300, 300",
        "20 PLOT 149, 300, 340",
        "30 PRINT POINT(340, 300); POINT(300, 260); POINT(300, 300)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         110\n");
}

#[test]
fn test_plot_144_moves_and_draws_no_circle() {
    let (result, output) = run(&[
        "10 MOVE 300, 300",
        "20 PLOT 144, 50, 0",
        "30 PRINT POINT(250, 300)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         0\n");
}

#[test]
fn test_plot_153_draws_a_filled_circle() {
    let (result, output) = run(&[
        "10 MOVE 300, 300",
        "20 PLOT 153, 50, 0",
        "30 PRINT POINT(300, 300); POINT(320, 320); POINT(340, 340)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         110\n");
}

#[test]
fn test_a_line_after_circle_starts_at_the_point_on_the_circle() {
    // BBC BASIC: CIRCLE 300, 300, 50 is MOVE 300, 300 : PLOT 145, 50, 0.
    // The last point is (350, 300), so the line goes to (360, 300).
    let (result, output) = run(&[
        "10 CIRCLE 300, 300, 50",
        "20 PLOT 1, 10, 0",
        "30 PRINT POINT(355, 300); POINT(305, 300)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         10\n");
}

#[test]
fn test_a_triangle_after_circle_uses_the_centre_and_the_point_on_the_circle() {
    // The triangle has the corners (300, 300), (350, 300) and (350, 350).
    // The point (340, 310) is inside it and is not on the circle.
    let (result, output) = run(&[
        "10 CIRCLE 300, 300, 50",
        "20 PLOT 85, 350, 350",
        "30 PRINT POINT(340, 310)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         1\n");
}
