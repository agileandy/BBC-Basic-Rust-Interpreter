//! Issue 138: a filled triangle with 2 corners on one row fills that row from
//! one corner to the other. A triangle with no flat edge draws the same pixels
//! as before.

use crate::common::run_with_limit;

#[test]
fn test_the_issue_program_fills_the_flat_bottom_edge() {
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : MOVE 100,100 : MOVE 300,100 : PLOT 85,200,300",
        "20 PRINT POINT(100,100); POINT(200,100); POINT(296,100); POINT(200,200)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1111\n");
}

#[test]
fn test_a_flat_bottom_edge_stops_at_the_corners() {
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : MOVE 100,100 : MOVE 300,100 : PLOT 85,200,300",
        "20 PRINT POINT(99,100); POINT(300,100); POINT(301,100); POINT(200,99)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0100\n");
}

#[test]
fn test_a_flat_top_edge_is_filled_from_corner_to_corner() {
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : MOVE 300,300 : MOVE 100,300 : PLOT 85,200,100",
        "20 PRINT POINT(100,300); POINT(200,300); POINT(300,300)",
        "30 PRINT POINT(99,300); POINT(301,300); POINT(200,301); POINT(200,200)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "111\n0001\n");
}

#[test]
fn test_a_flat_edge_with_the_target_as_a_corner_is_filled() {
    // The target is a corner of the flat edge, so the sort puts the corners
    // of the flat edge in a different order.
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : MOVE 200,300 : MOVE 300,100 : PLOT 85,100,100",
        "20 PRINT POINT(100,100); POINT(200,100); POINT(300,100); POINT(200,200)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1111\n");
}

#[test]
fn test_3_corners_on_one_row_fill_from_the_lowest_x_to_the_highest_x() {
    // The middle corner is the first corner, so neither end of the row is
    // the first corner.
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : MOVE 200,500 : MOVE 100,500 : PLOT 85,300,500",
        "20 PRINT POINT(100,500); POINT(150,500); POINT(250,500); POINT(300,500)",
        "30 PRINT POINT(99,500); POINT(301,500); POINT(200,501); POINT(200,499)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1111\n0000\n");
}

#[test]
fn test_a_triangle_with_no_flat_edge_draws_the_same_pixels_as_before() {
    // The values are from the code before issue 138.
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : MOVE 100,100 : MOVE 400,200 : PLOT 85,200,400",
        "20 PRINT POINT(100,100); POINT(400,200); POINT(200,400); POINT(250,250)",
        "30 PRINT POINT(101,100); POINT(100,101); POINT(399,200); POINT(200,399)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1111\n0111\n");
}
