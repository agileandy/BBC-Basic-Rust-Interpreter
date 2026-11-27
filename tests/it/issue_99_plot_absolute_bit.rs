//! Issue 99: bit 2 of the PLOT mode selects absolute (set) or relative
//! (clear) coordinates for each group of 8 modes.

use crate::common::run_lines as run;

#[test]
fn test_plot_69_sets_an_absolute_point() {
    let (result, output) = run(&[
        "10 MOVE 100, 100",
        "20 PLOT 69, 200, 200",
        "30 PRINT POINT(200, 200); POINT(300, 300)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "10\n");
}

#[test]
fn test_plot_65_sets_a_point_relative_to_the_last_point() {
    let (result, output) = run(&[
        "10 MOVE 100, 100",
        "20 PLOT 65, 10, 10",
        "30 PRINT POINT(110, 110); POINT(10, 10)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "10\n");
}

#[test]
fn test_plot_5_draws_an_absolute_line() {
    let (result, output) = run(&[
        "10 MOVE 100, 100",
        "20 PLOT 5, 200, 200",
        "30 PRINT POINT(200, 200); POINT(300, 300)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "10\n");
}

#[test]
fn test_plot_1_draws_a_line_relative_to_the_last_point() {
    let (result, output) = run(&[
        "10 MOVE 100, 100",
        "20 PLOT 1, 10, 10",
        "30 PRINT POINT(110, 110); POINT(10, 10)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "10\n");
}

#[test]
fn test_plot_85_fills_an_absolute_triangle() {
    // The triangle has the corners 100,100 and 300,100 and 200,300.
    let (result, output) = run(&[
        "10 MOVE 100, 100",
        "20 MOVE 300, 100",
        "30 PLOT 85, 200, 300",
        "40 PRINT POINT(200, 200); POINT(200, 300); POINT(500, 400)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "110\n");
}

#[test]
fn test_plot_81_fills_a_triangle_relative_to_the_last_point() {
    // The last corner is 300,100 plus -100,200, that is 200,300.
    let (result, output) = run(&[
        "10 MOVE 100, 100",
        "20 MOVE 300, 100",
        "30 PLOT 81, -100, 200",
        "40 PRINT POINT(200, 200); POINT(200, 300)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "11\n");
}

#[test]
fn test_a_relative_move_updates_the_two_last_points() {
    // PLOT 0 moves to 300,100, so the triangle is the same as for PLOT 85 above.
    let (result, output) = run(&[
        "10 MOVE 100, 100",
        "20 PLOT 0, 200, 0",
        "30 PLOT 85, 200, 300",
        "40 PRINT POINT(200, 200); POINT(200, 300)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "11\n");
}

#[test]
fn test_a_triangle_updates_the_two_last_points() {
    // The second triangle has the corners 300,100 and 200,300 and 400,300.
    let (result, output) = run(&[
        "10 MOVE 100, 100",
        "20 MOVE 300, 100",
        "30 PLOT 85, 200, 300",
        "40 PLOT 85, 400, 300",
        "50 PRINT POINT(300, 280); POINT(400, 300)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "11\n");
}

#[test]
fn test_plot_84_moves_without_a_triangle() {
    let (result, output) = run(&[
        "10 MOVE 100, 100",
        "20 MOVE 300, 100",
        "30 PLOT 84, 200, 300",
        "40 PRINT POINT(200, 200)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0\n");
}

#[test]
fn test_plot_132_moves_the_cursor_to_an_absolute_point() {
    // PLOT 65, 0, 0 sets a point at the last point, so it shows the cursor.
    let (result, output) = run(&[
        "10 MOVE 100, 100",
        "20 PLOT 132, 200, 200",
        "30 PLOT 65, 0, 0",
        "40 PRINT POINT(200, 200); POINT(300, 300)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "10\n");
}

#[test]
fn test_plot_128_moves_the_cursor_relative_to_the_last_point() {
    let (result, output) = run(&[
        "10 MOVE 100, 100",
        "20 PLOT 128, 10, 10",
        "30 PLOT 65, 0, 0",
        "40 PRINT POINT(110, 110); POINT(10, 10)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "10\n");
}

#[test]
fn test_a_move_only_group_reads_bit_2_as_absolute() {
    // The code moves only for the 8 group. PLOT 65, 0, 0 shows the cursor.
    let (result, output) = run(&[
        "10 MOVE 100, 100",
        "20 PLOT 12, 200, 200",
        "30 PLOT 65, 0, 0",
        "40 MOVE 100, 100",
        "50 PLOT 8, 10, 10",
        "60 PLOT 65, 0, 0",
        "70 PRINT POINT(200, 200); POINT(110, 110)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "11\n");
}
