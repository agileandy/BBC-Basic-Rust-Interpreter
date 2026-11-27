//! Issue 128: a filled triangle, a line, a circle outline, an ellipse and a
//! rectangle with very large coordinates return quickly and do not panic. A
//! shape that is on the canvas draws the same pixels as before.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use bbc_basic_interpreter::graphics::GraphicsSystem;

use crate::common::run_lines;

/// Do `run_lines` on a thread with a time limit of 2 seconds.
fn run_in_2_seconds(source: &[&str]) -> (Result<(), String>, String) {
    let lines: Vec<String> = source.iter().map(|line| line.to_string()).collect();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        let _ = sender.send(run_lines(&lines));
    });
    receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("program ends within 2 seconds and does not panic")
}

#[test]
fn test_the_issue_program_returns_and_fills_the_visible_part() {
    let (result, output) = run_in_2_seconds(&[
        "10 MODE 1 : MOVE 0,0 : MOVE 2000000000,0 : PLOT 85,0,2000000000",
        "20 PRINT POINT(10, 10)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "-1\n");
}

#[test]
fn test_a_triangle_partly_off_the_screen_draws_its_visible_part() {
    // The edge from (1000, 100) to the far vertex is at x = 800 on row 300.
    let (result, output) = run_in_2_seconds(&[
        "10 MOVE -2000000000, 100",
        "20 MOVE 1000, 100",
        "30 PLOT 85, -2000000000, 2000000000",
        "40 PRINT POINT(700, 300); POINT(900, 300); POINT(500, 50)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "-100\n");
}

#[test]
fn test_a_long_line_returns_and_draws_its_visible_part() {
    let (result, output) = run_in_2_seconds(&[
        "10 MOVE 0, 0",
        "20 DRAW 2000000000, 0",
        "30 MOVE -2000000000, 500",
        "40 DRAW 2000000000, 500",
        "50 PRINT POINT(100, 0); POINT(640, 500); POINT(640, 501)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "-1-10\n");
}

#[test]
fn test_a_large_circle_outline_returns_and_draws_its_visible_part() {
    // The circle goes through the point (1000, 500).
    let (result, output) = run_in_2_seconds(&[
        "10 CIRCLE 0, 0, 2000000000",
        "20 CIRCLE -1999999000, 500, 2000000000",
        "30 PRINT POINT(0, 0); POINT(1000, 500); POINT(999, 500)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0-10\n");
}

#[test]
fn test_a_circle_with_a_radius_larger_than_an_integer_does_not_panic() {
    let (result, output) = run_in_2_seconds(&[
        "10 MOVE -2000000000, 0",
        "20 PLOT 149, 2000000000, 0",
        "30 PRINT \"done\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "done\n");
}

#[test]
fn test_a_large_ellipse_returns_and_draws_its_visible_part() {
    // The top of the ellipse is at (0, 1000).
    let (result, output) = run_in_2_seconds(&[
        "10 ELLIPSE 0, 0, 2000000000, 1000",
        "20 PRINT POINT(0, 1000); POINT(0, 999)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "-10\n");
}

#[test]
fn test_an_ellipse_of_normal_size_on_the_screen_does_not_panic() {
    let (result, output) = run_in_2_seconds(&[
        "10 ELLIPSE 640, 512, 600, 400",
        "20 PRINT POINT(640, 912); POINT(640, 512)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "-10\n");
}

#[test]
fn test_a_large_rectangle_returns_and_fills_the_visible_part() {
    let (result, output) = run_in_2_seconds(&[
        "10 RECTANGLE 0, 0, 2000000000, 2000000000",
        "20 PRINT POINT(100, 100); POINT(1279, 1023)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "-1-1\n");
}

// The functions below are copies of the drawing algorithms before issue 128.
// Each one gives the points that the old algorithm sent to `set_pixel`, in
// order. The tests compare the new code with them on a small canvas.

fn old_line(x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<(i32, i32)> {
    let mut points = Vec::new();
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;
    let mut x = x0;
    let mut y = y0;
    loop {
        points.push((x, y));
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }
    points
}

fn old_circle_outline(cx: i32, cy: i32, radius: i32) -> Vec<(i32, i32)> {
    let mut points = Vec::new();
    if radius <= 0 {
        return points;
    }
    let mut x = radius;
    let mut y = 0;
    let mut err = 0;
    while x >= y {
        points.extend([
            (cx + x, cy + y),
            (cx + y, cy + x),
            (cx - y, cy + x),
            (cx - x, cy + y),
            (cx - x, cy - y),
            (cx - y, cy - x),
            (cx + y, cy - x),
            (cx + x, cy - y),
        ]);
        y += 1;
        if err <= 0 {
            err += 2 * y + 1;
        } else {
            x -= 1;
            err += 2 * (y - x) + 1;
        }
    }
    points
}

fn old_ellipse(cx: i32, cy: i32, rx: i32, ry: i32) -> Vec<(i32, i32)> {
    let mut points = Vec::new();
    if rx <= 0 || ry <= 0 {
        return points;
    }
    let quadrants = |points: &mut Vec<(i32, i32)>, x: i32, y: i32| {
        points.extend([
            (cx + x, cy + y),
            (cx - x, cy + y),
            (cx + x, cy - y),
            (cx - x, cy - y),
        ]);
    };
    let mut x = 0;
    let mut y = ry;
    let rx_sq = rx * rx;
    let ry_sq = ry * ry;
    let two_rx_sq = 2 * rx_sq;
    let two_ry_sq = 2 * ry_sq;
    let mut px = 0;
    let mut py = two_rx_sq * y;
    quadrants(&mut points, x, y);
    let mut p = ry_sq - (rx_sq * ry) + (rx_sq / 4);
    while px < py {
        x += 1;
        px += two_ry_sq;
        if p < 0 {
            p += ry_sq + px;
        } else {
            y -= 1;
            py -= two_rx_sq;
            p += ry_sq + px - py;
        }
        quadrants(&mut points, x, y);
    }
    p = ry_sq * (x + 1) * (x + 1) + rx_sq * (y - 1) * (y - 1) - rx_sq * ry_sq;
    while y > 0 {
        y -= 1;
        py -= two_rx_sq;
        if p > 0 {
            p += rx_sq - py;
        } else {
            x += 1;
            px += two_ry_sq;
            p += rx_sq - py + px;
        }
        quadrants(&mut points, x, y);
    }
    points
}

fn old_rectangle(x1: i32, y1: i32, x2: i32, y2: i32, filled: bool) -> Vec<(i32, i32)> {
    let mut points = Vec::new();
    let (min_x, max_x) = (x1.min(x2), x1.max(x2));
    let (min_y, max_y) = (y1.min(y2), y1.max(y2));
    if filled {
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                points.push((x, y));
            }
        }
    } else {
        for x in min_x..=max_x {
            points.push((x, min_y));
            points.push((x, max_y));
        }
        for y in min_y..=max_y {
            points.push((min_x, y));
            points.push((max_x, y));
        }
    }
    points
}

fn old_fill_triangle(x1: i32, y1: i32, x2: i32, y2: i32, x3: i32, y3: i32) -> Vec<(i32, i32)> {
    let mut points = Vec::new();
    let mut verts = [(x1, y1), (x2, y2), (x3, y3)];
    verts.sort_by_key(|v| v.1);
    let (x1, y1) = verts[0];
    let (x2, y2) = verts[1];
    let (x3, y3) = verts[2];
    for y in y1..=y3 {
        let mut x_left = x1;
        let mut x_right = x1;
        if y <= y2 {
            if y2 != y1 {
                x_left = x1 + (x2 - x1) * (y - y1) / (y2 - y1);
            }
            if y3 != y1 {
                x_right = x1 + (x3 - x1) * (y - y1) / (y3 - y1);
            }
        } else {
            if y3 != y2 {
                x_left = x2 + (x3 - x2) * (y - y2) / (y3 - y2);
            }
            if y3 != y1 {
                x_right = x1 + (x3 - x1) * (y - y1) / (y3 - y1);
            }
        }
        if x_left > x_right {
            std::mem::swap(&mut x_left, &mut x_right);
        }
        for x in x_left..=x_right {
            points.push((x, y));
        }
    }
    points
}

const WIDTH: usize = 64;
const HEIGHT: usize = 48;

/// A small pseudo-random number generator, so that each run uses the same cases.
struct Numbers(u64);

impl Numbers {
    /// A number from `low` to `high`, both included.
    fn between(&mut self, low: i32, high: i32) -> i32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let span = (high - low + 1) as u64;
        low + ((self.0 >> 33) % span) as i32
    }
}

/// Draw with `draw` on a new canvas in XOR mode and compare each pixel with
/// the old points. XOR mode also shows a point that is sent 2 times.
fn assert_same_pixels(
    case: &str,
    origin: (i32, i32),
    old_points: &[(i32, i32)],
    draw: impl Fn(&mut GraphicsSystem),
) {
    let mut expected = vec![vec![false; WIDTH]; HEIGHT];
    for &(x, y) in old_points {
        let canvas_x = x + origin.0;
        let canvas_y = HEIGHT as i32 - 1 - (y + origin.1);
        if (0..WIDTH as i32).contains(&canvas_x) && (0..HEIGHT as i32).contains(&canvas_y) {
            let pixel = &mut expected[canvas_y as usize][canvas_x as usize];
            *pixel = !*pixel;
        }
    }

    let mut graphics = GraphicsSystem::with_dimensions(WIDTH, HEIGHT);
    graphics.set_origin(origin.0, origin.1);
    graphics.set_color(3, 1);
    draw(&mut graphics);

    for (canvas_y, row) in expected.iter().enumerate() {
        for (canvas_x, &pixel) in row.iter().enumerate() {
            let x = canvas_x as i32 - origin.0;
            let y = HEIGHT as i32 - 1 - canvas_y as i32 - origin.1;
            assert_eq!(
                graphics.get_pixel(x, y),
                Some(pixel),
                "{case}: pixel ({x}, {y}) is not the same as before"
            );
        }
    }
}

#[test]
fn test_lines_draw_the_same_pixels_as_before() {
    let mut numbers = Numbers(128);
    for _ in 0..3000 {
        let origin = (numbers.between(-8, 8), numbers.between(-8, 8));
        let (x0, y0) = (numbers.between(-40, 100), numbers.between(-40, 90));
        let (x1, y1) = (numbers.between(-40, 100), numbers.between(-40, 90));
        assert_same_pixels(
            &format!("line ({x0}, {y0}) to ({x1}, {y1}), origin {origin:?}"),
            origin,
            &old_line(x0, y0, x1, y1),
            |graphics| {
                graphics.move_to(x0, y0);
                graphics.draw_line_to(x1, y1);
            },
        );
    }
}

#[test]
fn test_circle_outlines_draw_the_same_pixels_as_before() {
    let mut numbers = Numbers(129);
    for _ in 0..2000 {
        let origin = (numbers.between(-8, 8), numbers.between(-8, 8));
        let (cx, cy) = (numbers.between(-30, 90), numbers.between(-30, 80));
        let radius = numbers.between(-5, 60);
        assert_same_pixels(
            &format!("circle ({cx}, {cy}) radius {radius}, origin {origin:?}"),
            origin,
            &old_circle_outline(cx, cy, radius.abs()),
            |graphics| graphics.draw_circle(cx, cy, radius),
        );
    }
}

#[test]
fn test_ellipses_draw_the_same_pixels_as_before() {
    let mut numbers = Numbers(130);
    for _ in 0..2000 {
        let origin = (numbers.between(-8, 8), numbers.between(-8, 8));
        let (cx, cy) = (numbers.between(-30, 90), numbers.between(-30, 80));
        let (rx, ry) = (numbers.between(-2, 60), numbers.between(-2, 60));
        // Since issue 126, ELLIPSE is MOVE, PLOT 0 and PLOT 193, and each
        // radius is a distance between 2 points. Thus a negative radius
        // draws the same ellipse as the positive radius.
        assert_same_pixels(
            &format!("ellipse ({cx}, {cy}) radii {rx}, {ry}, origin {origin:?}"),
            origin,
            &old_ellipse(cx, cy, rx.abs(), ry.abs()),
            |graphics| graphics.draw_ellipse(cx, cy, rx, ry),
        );
    }
}

#[test]
fn test_rectangles_draw_the_same_pixels_as_before() {
    let mut numbers = Numbers(131);
    for _ in 0..2000 {
        let origin = (numbers.between(-8, 8), numbers.between(-8, 8));
        let (x1, y1) = (numbers.between(-40, 100), numbers.between(-40, 90));
        let (x2, y2) = (numbers.between(-40, 100), numbers.between(-40, 90));
        let filled = numbers.between(0, 1) == 1;
        assert_same_pixels(
            &format!("rectangle ({x1}, {y1}) to ({x2}, {y2}) filled {filled}, origin {origin:?}"),
            origin,
            &old_rectangle(x1, y1, x2, y2, filled),
            |graphics| graphics.draw_rectangle(x1, y1, x2, y2, filled),
        );
    }
}

#[test]
fn test_filled_triangles_draw_the_same_pixels_as_before() {
    let mut numbers = Numbers(132);
    for _ in 0..3000 {
        let origin = (numbers.between(-8, 8), numbers.between(-8, 8));
        let (x1, y1) = (numbers.between(-40, 100), numbers.between(-40, 90));
        let (x2, y2) = (numbers.between(-40, 100), numbers.between(-40, 90));
        let (x3, y3) = (numbers.between(-40, 100), numbers.between(-40, 90));
        assert_same_pixels(
            &format!("triangle ({x1}, {y1}), ({x2}, {y2}), ({x3}, {y3}), origin {origin:?}"),
            origin,
            &old_fill_triangle(x1, y1, x2, y2, x3, y3),
            |graphics| graphics.draw_triangle(x1, y1, x2, y2, x3, y3, true),
        );
    }
}
