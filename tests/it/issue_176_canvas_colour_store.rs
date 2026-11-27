//! Issue 176: the graphics canvas stores the colour number of each pixel.
//! GCOL sets the graphics foreground or background colour and the action,
//! CLG fills with the background, POINT gives the colour number, and FILL
//! fills the background area with the foreground colour.

use crate::common::run_with_limit;
use bbc_basic_interpreter::graphics::{GraphicsSystem, ScreenMode};

#[test]
fn test_the_canvas_view_gives_the_size_and_the_colour_with_the_first_row_at_the_top() {
    let mut graphics = GraphicsSystem::with_dimensions(10, 8);
    graphics.reset_for_mode(&ScreenMode::from_number(1).expect("mode 1"));
    graphics.set_color(0, 2);
    // PLOT 69 at x 3, y 0: the bottom row of the canvas
    graphics.plot(69, 3, 0);

    assert_eq!(graphics.canvas_size(), (10, 8));
    assert_eq!(graphics.colour_at(3, 7), 2);
    assert_eq!(graphics.colour_at(3, 0), 0);
    assert_eq!(graphics.colour_at(10, 0), 0);
}

#[test]
fn test_mode_1_plots_in_colour_3_by_default() {
    let (result, output) =
        run_with_limit(&["10 MODE 1 : PLOT 69, 100, 100 : PRINT POINT(100, 100)"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "3\n");
}

#[test]
fn test_gcol_0_1_plots_in_colour_1() {
    let (result, output) =
        run_with_limit(&["10 MODE 1 : GCOL 0, 1 : PLOT 69, 100, 100 : PRINT POINT(100, 100)"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n");
}

#[test]
fn test_gcol_0_0_clears_a_pixel() {
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : PLOT 69, 100, 100 : GCOL 0, 0 : PLOT 69, 100, 100",
        "20 PRINT POINT(100, 100)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0\n");
}

#[test]
fn test_gcol_3_1_on_a_pixel_of_colour_2_gives_3() {
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : GCOL 0, 2 : PLOT 69, 100, 100",
        "20 GCOL 3, 1 : PLOT 69, 100, 100 : PRINT POINT(100, 100)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "3\n");
}

#[test]
fn test_mode_0_plots_in_colour_1_by_default() {
    let (result, output) =
        run_with_limit(&["10 MODE 0 : PLOT 69, 100, 100 : PRINT POINT(100, 100)"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n");
}

#[test]
fn test_gcol_0_5_in_a_4_colour_mode_gives_colour_1() {
    let (result, output) =
        run_with_limit(&["10 MODE 1 : GCOL 0, 5 : PLOT 69, 100, 100 : PRINT POINT(100, 100)"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n");
}

#[test]
fn test_gcol_0_129_and_clg_fill_with_colour_1() {
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : GCOL 0, 129 : CLG",
        "20 PRINT POINT(0, 0); POINT(640, 512); POINT(1279, 1023)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "111\n");
}

#[test]
fn test_fill_stops_at_a_line_of_a_different_colour() {
    // The outline is colour 2, the fill is colour 1 and the background is 0
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : GCOL 0, 2 : RECTANGLE 100, 100, 200, 200",
        "20 GCOL 0, 1 : FILL 200, 200",
        "30 PRINT POINT(200, 200); POINT(101, 299); POINT(100, 200); POINT(50, 50)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1120\n");
}

#[test]
fn test_fill_from_a_pixel_that_is_not_the_background_fills_nothing() {
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : GCOL 0, 2 : RECTANGLE 100, 100, 200, 200",
        "20 GCOL 0, 1 : FILL 100, 200",
        "30 PRINT POINT(100, 200); POINT(200, 100)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "22\n");
}

#[test]
fn test_fill_fills_the_pixels_of_the_graphics_background_colour() {
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : GCOL 0, 129 : CLG",
        "20 GCOL 0, 2 : RECTANGLE 100, 100, 200, 200",
        "30 GCOL 0, 3 : FILL 200, 200",
        "40 PRINT POINT(200, 200); POINT(100, 200); POINT(50, 50)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "321\n");
}

#[test]
fn test_fill_with_an_action_that_keeps_the_background_stops() {
    // 0 AND 3 is 0, thus the fill changes no pixel
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : GCOL 2, 3 : FILL 200, 200",
        "20 PRINT POINT(200, 200)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0\n");
}

#[test]
fn test_gcol_129_does_not_change_the_foreground_colour() {
    let (result, output) =
        run_with_limit(&["10 MODE 1 : GCOL 0, 129 : PLOT 69, 100, 100 : PRINT POINT(100, 100)"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "3\n");
}
