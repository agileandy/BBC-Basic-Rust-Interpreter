//! Issue 126: ELLIPSE records the points that BBC BASIC gives, and the PLOT
//! codes 192 to 207 draw an ellipse.
//!
//! The BBC BASIC for Windows manual gives:
//! "ELLIPSE x,y,a,b is equivalent to MOVE x,y : PLOT 0,a,0 : PLOT 193,0,b".
//! For PLOT 192 to 207, the point before the last point is the centre, the
//! last point is on a vertical tangent and the target is on a horizontal
//! tangent.

use crate::common::{run_lines as run, run_with_limit};

/// A program that prints one number for the set pixels of the box from
/// (180, 230) to (420, 370), after the drawing lines.
fn pixel_sum(drawing: &[&str]) -> (Result<(), String>, String) {
    let mut lines: Vec<&str> = drawing.to_vec();
    lines.extend([
        "100 S = 0",
        "110 FOR Y% = 230 TO 370",
        "120 FOR X% = 180 TO 420",
        "130 IF POINT(X%, Y%) THEN S = S + X% * 1000 + Y%",
        "140 NEXT X%",
        "150 NEXT Y%",
        "160 PRINT S",
    ]);
    run(&lines)
}

#[test]
fn test_a_line_after_ellipse_starts_at_the_point_of_the_rule() {
    // After ELLIPSE 300, 300, 100, 50 the last point is (400, 350), so the
    // line goes from (400, 350) to (410, 350).
    let (result, output) = run(&[
        "10 ELLIPSE 300, 300, 100, 50",
        "20 PLOT 1, 10, 0",
        "30 PRINT POINT(405, 350); POINT(305, 300)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "-10\n");
}

#[test]
fn test_a_triangle_after_ellipse_uses_the_2_points_of_the_rule() {
    // The triangle has the corners (400, 300), (400, 350) and (450, 300).
    // The point (410, 310) is inside it and is not on the ellipse.
    let (result, output) = run(&[
        "10 ELLIPSE 300, 300, 100, 50",
        "20 PLOT 85, 450, 300",
        "30 PRINT POINT(410, 310)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "-1\n");
}

#[test]
fn test_plot_193_draws_an_ellipse_outline() {
    let (result, output) = run(&[
        "10 MOVE 300, 300",
        "20 PLOT 0, 100, 0",
        "30 PLOT 193, 0, 50",
        "40 PRINT POINT(400, 300); POINT(200, 300); POINT(300, 350); POINT(300, 250)",
        "50 PRINT POINT(300, 300); POINT(350, 300)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "-1-1-1-1\n00\n");
}

#[test]
fn test_plot_193_gives_the_pixels_of_ellipse() {
    let (ellipse_result, ellipse_output) = pixel_sum(&["10 ELLIPSE 300, 300, 100, 50"]);
    let (plot_result, plot_output) = pixel_sum(&[
        "10 MOVE 300, 300",
        "20 PLOT 0, 100, 0",
        "30 PLOT 193, 0, 50",
    ]);

    assert_eq!(ellipse_result, Ok(()));
    assert_eq!(plot_result, Ok(()));
    assert_ne!(ellipse_output, "0\n");
    assert_eq!(plot_output, ellipse_output);
}

#[test]
fn test_ellipse_pixels_are_the_same_as_before_the_change() {
    // The sum and the count of the set pixels on `main` at ecb0775.
    let (result, output) = run(&[
        "10 ELLIPSE 300, 300, 100, 50",
        "15 ELLIPSE 600, 500, 37, 141",
        "100 S = 0",
        "110 FOR Y% = 0 TO 700",
        "120 FOR X% = 150 TO 700",
        "130 IF POINT(X%, Y%) THEN S = S + X% * 1000 + Y% : C% = C% + 1",
        "140 NEXT X%",
        "150 NEXT Y%",
        "160 PRINT S; \" \"; C%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "485226400 1032\n");
}

#[test]
fn test_ellipse_with_negative_radii_draws_the_same_ellipse() {
    // Before the change, a negative radius drew nothing. The rule gives the
    // radii as distances between the points, so the sign has no effect.
    let (result, output) = run(&[
        "10 ELLIPSE 300, 300, -100, -50",
        "20 PRINT POINT(400, 300); POINT(200, 300); POINT(300, 350); POINT(300, 250)",
        "30 PLOT 1, 10, 0",
        "40 PRINT POINT(205, 250)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "-1-1-1-1\n-1\n");
}

#[test]
fn test_plot_197_uses_an_absolute_point_on_a_horizontal_tangent() {
    // The example of the manual: the centre is (250, 200), the horizontal
    // radius is 250 and the vertical radius is 150.
    let (result, output) = run(&[
        "10 MOVE 250, 200",
        "20 MOVE 500, 300",
        "30 PLOT 197, 150, 350",
        "40 PRINT POINT(500, 200); POINT(250, 350); POINT(250, 50); POINT(250, 200)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "-1-1-10\n");
}

#[test]
fn test_plot_192_and_196_only_move() {
    let (result, output) = run(&[
        "10 MOVE 300, 300",
        "20 PLOT 0, 100, 0",
        "30 PLOT 192, 0, 50",
        "40 PRINT POINT(200, 300); POINT(300, 350)",
        "50 MOVE 300, 300",
        "60 MOVE 400, 300",
        "70 PLOT 196, 300, 350",
        "80 PRINT POINT(200, 300); POINT(300, 350)",
        "90 PLOT 1, 10, 0",
        "100 PRINT POINT(305, 350)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "00\n00\n-1\n");
}

#[test]
fn test_plot_194_and_195_only_move() {
    let (result, output) = run(&[
        "10 MOVE 300, 300",
        "20 PLOT 0, 100, 0",
        "30 PLOT 194, 0, 50",
        "40 MOVE 300, 300",
        "50 PLOT 0, 100, 0",
        "60 PLOT 195, 0, 50",
        "70 PRINT POINT(200, 300); POINT(300, 350)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "00\n");
}

#[test]
fn test_plot_201_draws_a_filled_ellipse() {
    // (380, 300) and (300, 340) are inside. (380, 340) is outside.
    let (result, output) = run(&[
        "10 MOVE 300, 300",
        "20 PLOT 0, 100, 0",
        "30 PLOT 201, 0, 50",
        "40 PRINT POINT(300, 300); POINT(380, 300); POINT(300, 340); POINT(380, 340)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "-1-1-10\n");
}

#[test]
fn test_plot_205_draws_a_filled_ellipse_through_absolute_points() {
    let (result, output) = run(&[
        "10 MOVE 250, 200",
        "20 MOVE 500, 300",
        "30 PLOT 205, 150, 350",
        "40 PRINT POINT(250, 200); POINT(450, 200); POINT(250, 320)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "-1-1-1\n");
}

#[test]
fn test_plot_200_only_moves() {
    let (result, output) = run(&[
        "10 MOVE 300, 300",
        "20 PLOT 0, 100, 0",
        "30 PLOT 200, 0, 50",
        "40 PRINT POINT(300, 300)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0\n");
}

#[test]
fn test_a_large_filled_ellipse_is_clipped_to_the_canvas() {
    // Only the rows and the columns on the canvas are visited, so a large
    // filled ellipse is fast and does not panic.
    let (result, output) = run_with_limit(&[
        "10 MOVE 300, 300",
        "20 PLOT 0, 30000, 0",
        "30 PLOT 201, 0, 30000",
        "40 PRINT POINT(0, 0); POINT(300, 300)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "-1-1\n");
}

#[test]
fn test_a_large_ellipse_outline_does_not_panic() {
    let (result, output) = run_with_limit(&["10 ELLIPSE 300, 300, 30000, 20000", "20 PRINT 1"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n");
}
