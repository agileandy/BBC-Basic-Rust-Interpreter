//! Issue 137: a drawing statement that adds 2 very large coordinates returns
//! with no panic. A stored point (the last point) saturates at the i32
//! limits. A statement with normal values draws the same pixels as before.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use bbc_basic_interpreter::graphics::GraphicsSystem;

use crate::common::run_lines;

/// Do `run_lines` on a thread with a time limit of 10 seconds.
fn run_in_10_seconds(source: &[&str]) -> (Result<(), String>, String) {
    let lines: Vec<String> = source.iter().map(|line| line.to_string()).collect();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        let _ = sender.send(run_lines(&lines));
    });
    receiver
        .recv_timeout(Duration::from_secs(10))
        .expect("program ends within 10 seconds and does not panic")
}

#[test]
fn test_a_relative_plot_past_the_i32_limit_returns() {
    let (result, output) = run_in_10_seconds(&[
        "10 MODE 1 : MOVE 2000000000,0 : PLOT 1,2000000000,0",
        "20 PRINT \"done\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "done\n");
}

#[test]
fn test_a_relative_plot_past_the_i32_limit_saturates_the_last_point() {
    // The last point saturates at 2147483647. Thus a relative move of
    // -2147483647 goes to x = 0, and a relative point at 10, 10 is at 10, 10.
    let (result, output) = run_in_10_seconds(&[
        "10 MODE 1 : MOVE 2000000000,0 : PLOT 1,2000000000,0",
        "20 PLOT 0,-2147483647,0 : PLOT 65,10,10",
        "30 PRINT POINT(10,10)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "-1\n");
}

#[test]
fn test_a_relative_plot_below_the_i32_limit_saturates_the_last_point() {
    let (result, output) = run_in_10_seconds(&[
        "10 MODE 1 : MOVE -2000000000,0 : PLOT 0,-2000000000,0",
        "20 PLOT 0,2147483647,0 : PLOT 65,10,10",
        "30 PRINT POINT(9,10)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "-1\n");
}

#[test]
fn test_a_rectangle_past_the_i32_limit_returns() {
    let (result, output) = run_in_10_seconds(&[
        "10 MODE 1 : RECTANGLE 2000000000,0,2000000000,10",
        "20 RECTANGLE -2000000000,0,-2000000000,10",
        "30 PRINT \"done\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "done\n");
}

#[test]
fn test_a_point_plot_with_a_large_origin_returns() {
    let (result, output) = run_in_10_seconds(&[
        "10 MODE 1 : ORIGIN 2000000000,0 : PLOT 69,2000000000,0",
        "20 ORIGIN -2000000000,-2000000000 : PLOT 69,-2000000000,-2000000000",
        "30 PRINT \"done\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "done\n");
}

#[test]
fn test_point_with_a_large_origin_returns_false() {
    let (result, output) = run_in_10_seconds(&[
        "10 MODE 1 : ORIGIN 2000000000,0",
        "20 PRINT POINT(2000000000,0)",
        "30 ORIGIN 0,2000000000",
        "40 PRINT POINT(0,2000000000)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0\n0\n");
}

#[test]
fn test_a_circle_past_the_i32_limit_returns() {
    let (result, output) = run_in_10_seconds(&[
        "10 MODE 1 : CIRCLE 2000000000,0,2000000000",
        "20 CIRCLE -2000000000,0,-2000000000",
        "30 PRINT \"done\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "done\n");
}

#[test]
fn test_an_ellipse_past_the_i32_limit_returns() {
    let (result, output) = run_in_10_seconds(&[
        "10 MODE 1 : ELLIPSE 2000000000,2000000000,2000000000,2000000000",
        "20 PRINT \"done\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "done\n");
}

#[test]
fn test_a_fill_at_the_i32_limit_returns() {
    // The origin puts the point 2147483647, 2147483647 on the canvas. The
    // fill must not add 1 to it.
    let (result, output) = run_in_10_seconds(&[
        "10 MODE 1 : ORIGIN -2147483647,-2147483647",
        "20 FILL 2147483647,2147483647",
        "30 PRINT POINT(2147483647,2147483647)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "-1\n");
}

#[test]
fn test_move_by_and_draw_line_by_saturate_the_last_point() {
    let mut graphics = GraphicsSystem::with_dimensions(100, 100);
    graphics.move_to(2_000_000_000, -2_000_000_000);
    graphics.move_by(2_000_000_000, -2_000_000_000);
    assert_eq!(graphics.get_position(), (i32::MAX, i32::MIN));

    graphics.move_to(-2_000_000_000, 2_000_000_000);
    graphics.draw_line_by(-2_000_000_000, 2_000_000_000);
    assert_eq!(graphics.get_position(), (i32::MIN, i32::MAX));
}

#[test]
fn test_a_relative_plot_with_normal_values_draws_the_same_pixels() {
    let (result, output) = run_in_10_seconds(&[
        "10 MODE 1 : MOVE 100,100 : PLOT 1,50,0",
        "20 PRINT POINT(100,100); POINT(125,100); POINT(150,100); POINT(151,100)",
        "30 PLOT 65,-10,5",
        "40 PRINT POINT(140,105); POINT(141,105)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "-1-1-10\n-10\n");
}

#[test]
fn test_a_rectangle_with_normal_values_draws_the_same_pixels() {
    // RECTANGLE FILL draws a filled rectangle from x, y to x + width, y + height.
    let (result, output) = run_in_10_seconds(&[
        "10 MODE 1 : RECTANGLE FILL 10,10,20,30",
        "20 PRINT POINT(10,10); POINT(30,40); POINT(30,10); POINT(20,20); POINT(31,40); POINT(10,41)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "-1-1-1-100\n");
}

#[test]
fn test_a_circle_with_normal_values_draws_the_same_pixels() {
    let (result, output) = run_in_10_seconds(&[
        "10 MODE 1 : CIRCLE 200,200,50",
        "20 PRINT POINT(250,200); POINT(200,250); POINT(150,200); POINT(200,200); POINT(251,200)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "-1-1-100\n");
}
