//! Issue 161: POINT gives the colour number of a pixel, and -1 for a point
//! off the screen. These tests use MODE 0, which has 2 colours, thus a set
//! pixel gives 1 and a clear pixel gives 0 (issue 176 stores the colour).

use crate::common::run_lines;

#[test]
fn test_point_gives_1_for_a_set_pixel_in_a_2_colour_mode() {
    let (result, output) = run_lines(&[
        "10 MODE 0",
        "20 PLOT 69, 400, 400",
        "30 PRINT POINT(400, 400)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n");
}

#[test]
fn test_point_gives_0_for_a_clear_pixel() {
    let (result, output) = run_lines(&["10 MODE 0", "20 PRINT POINT(100, 100)"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0\n");
}

#[test]
fn test_point_gives_minus_1_off_the_screen_in_each_direction() {
    let (result, output) = run_lines(&[
        "10 MODE 0",
        "20 PRINT POINT(-1, 500)",
        "30 PRINT POINT(1280, 500)",
        "40 PRINT POINT(500, -1)",
        "50 PRINT POINT(500, 1024)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "-1\n-1\n-1\n-1\n");
}

#[test]
fn test_point_at_the_screen_edge_is_on_the_screen() {
    let (result, output) = run_lines(&[
        "10 MODE 0",
        "20 PRINT POINT(0, 0)",
        "30 PRINT POINT(1279, 1023)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0\n0\n");
}

#[test]
fn test_point_off_the_screen_uses_the_origin() {
    // With the origin at 100, 100 the screen is -100 to 1179 in x.
    let (result, output) = run_lines(&[
        "10 MODE 0",
        "20 ORIGIN 100, 100",
        "30 PRINT POINT(-100, -100)",
        "40 PRINT POINT(-101, 0)",
        "50 PRINT POINT(1180, 0)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0\n-1\n-1\n");
}

#[test]
fn test_point_minus_10_minus_10_gives_minus_1() {
    let (result, output) = run_lines(&[
        "10 PLOT 69, 400, 400",
        "20 PRINT POINT(400, 400)",
        "30 PRINT POINT(-10, -10)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n-1\n");
}

#[test]
fn test_if_point_then_works_for_a_set_pixel() {
    let (result, output) = run_lines(&[
        "10 MODE 0",
        "20 PLOT 69, 400, 400",
        "30 IF POINT(400, 400) THEN PRINT \"set\" ELSE PRINT \"clear\"",
        "40 IF POINT(100, 100) THEN PRINT \"set\" ELSE PRINT \"clear\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "set\nclear\n");
}

#[test]
fn test_point_gives_0_for_a_pixel_drawn_in_colour_0() {
    let (result, output) = run_lines(&[
        "10 MODE 0",
        "20 PLOT 69, 400, 400",
        "30 GCOL 0, 0",
        "40 PLOT 69, 400, 400",
        "50 PRINT POINT(400, 400)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0\n");
}
