//! Issue 184: FILL x, y is the same as PLOT 133, x, y. Thus FILL moves the
//! last point to x, y, and the old last point becomes the point before it.

use crate::common::run_with_limit;

#[test]
fn test_a_relative_point_after_fill_starts_from_the_fill_point() {
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : MOVE 100, 100 : FILL 300, 300 : GCOL 0, 2 : PLOT 65, 0, 0",
        "20 PRINT POINT(300, 300); POINT(100, 100)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "23\n");
}

/// A square outline, a move outside it, a fill inside it, then a filled
/// triangle with the 2 last points. The triangle is (800, 100), (300, 300)
/// and (800, 300) only if the fill moves the last point.
fn fill_then_triangle(fill: &str) -> (Result<(), String>, String) {
    let line_20 = format!("20 MOVE 800, 100 : {fill} : GCOL 0, 2 : PLOT 85, 800, 300");
    run_with_limit(&[
        "10 MODE 1 : MOVE 100, 100 : DRAW 500, 100 : DRAW 500, 500 : DRAW 100, 500 : DRAW 100, 100",
        &line_20,
        "30 PRINT POINT(300, 300); POINT(200, 450); POINT(700, 280); POINT(750, 200)",
        "40 PRINT POINT(50, 50); POINT(1000, 800)",
    ])
}

#[test]
fn test_fill_and_plot_133_give_the_same_pixels_and_the_same_last_2_points() {
    let fill = fill_then_triangle("FILL 300, 300");
    let plot = fill_then_triangle("PLOT 133, 300, 300");

    assert_eq!(fill.0, Ok(()));
    assert_eq!(fill.1, "2322\n00\n");
    assert_eq!(fill, plot);
}

#[test]
fn test_fill_in_a_text_only_mode_draws_nothing_as_plot_133_does() {
    let (result, output) = run_with_limit(&[
        "10 MODE 7 : MOVE 100, 100 : FILL 300, 300 : PLOT 133, 300, 300",
        "20 PRINT \"done\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "done\n");
}
