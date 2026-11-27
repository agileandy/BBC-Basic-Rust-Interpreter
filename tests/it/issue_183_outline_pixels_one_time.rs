//! Issue 183: a circle or an ellipse outline sets each pixel 1 time. Thus an
//! outline in the inverse colour, drawn 2 times, leaves a clear canvas. With
//! the store action, the pixels are the same as before.

use std::collections::BTreeSet;

use bbc_basic_interpreter::graphics::GraphicsSystem;

use crate::common::run_lines;
use crate::issue_128_triangle_fill_clip::{old_circle_outline, old_ellipse};

const WIDTH: usize = 64;
const HEIGHT: usize = 48;

/// The GCOL action that inverts the old colour
const INVERT: u8 = 4;

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

/// The points of the canvas that have a colour that is not 0
fn set_points(graphics: &GraphicsSystem, origin: (i32, i32)) -> BTreeSet<(i32, i32)> {
    let mut points = BTreeSet::new();
    for canvas_y in 0..HEIGHT as i32 {
        for canvas_x in 0..WIDTH as i32 {
            let (x, y) = (canvas_x - origin.0, canvas_y - origin.1);
            if graphics.get_pixel(x, y) == Some(true) {
                points.insert((x, y));
            }
        }
    }
    points
}

/// A new small canvas with the origin and the GCOL action
fn canvas(origin: (i32, i32), action: u8) -> GraphicsSystem {
    let mut graphics = GraphicsSystem::with_dimensions(WIDTH, HEIGHT);
    graphics.set_origin(origin.0, origin.1);
    graphics.set_color(action, 1);
    graphics
}

/// Draw with `draw` 2 times with the invert action, and check that the canvas
/// is clear. A pixel that one drawing sets 2 times stays set.
fn assert_clear_after_2_inverts(
    case: &str,
    origin: (i32, i32),
    draw: impl Fn(&mut GraphicsSystem),
) {
    let mut graphics = canvas(origin, INVERT);
    draw(&mut graphics);
    draw(&mut graphics);
    let left = set_points(&graphics, origin);
    assert!(left.is_empty(), "{case}: points {left:?} are set");
}

/// Draw with `draw` 1 time with the invert action, and check that each old
/// point is set. A pixel that the drawing sets 2 times is clear.
fn assert_one_invert_sets(
    case: &str,
    origin: (i32, i32),
    expected: &BTreeSet<(i32, i32)>,
    draw: impl Fn(&mut GraphicsSystem),
) {
    let mut graphics = canvas(origin, INVERT);
    draw(&mut graphics);
    assert_eq!(&set_points(&graphics, origin), expected, "{case}");
}

/// The old points that are on the canvas
fn on_canvas(points: &[(i32, i32)], origin: (i32, i32)) -> BTreeSet<(i32, i32)> {
    points
        .iter()
        .copied()
        .filter(|&(x, y)| {
            (0..WIDTH as i32).contains(&(x + origin.0))
                && (0..HEIGHT as i32).contains(&(y + origin.1))
        })
        .collect()
}

#[test]
fn test_the_issue_program_prints_22() {
    let (result, output) = run_lines(&[
        "10 MODE 1 : GCOL 0, 1 : MOVE 300, 300 : PLOT 149, 350, 300",
        "20 MOVE 300, 300 : PLOT 150, 350, 300 : PRINT POINT(350, 300); POINT(348, 310)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "22\n");
}

#[test]
fn test_an_ellipse_in_the_inverse_colour_sets_its_axis_points() {
    let (result, output) = run_lines(&[
        "10 MODE 1 : GCOL 0, 1 : MOVE 300, 300 : MOVE 350, 300 : PLOT 197, 300, 330",
        "20 MOVE 300, 300 : MOVE 350, 300 : PLOT 198, 300, 330",
        "30 PRINT POINT(350, 300); POINT(300, 330); POINT(250, 300); POINT(300, 270)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "2222\n");
}

#[test]
fn test_a_circle_with_radius_1_sets_8_points_1_time() {
    let expected = BTreeSet::from([
        (9, 10),
        (11, 10),
        (10, 9),
        (10, 11),
        (9, 9),
        (9, 11),
        (11, 9),
        (11, 11),
    ]);
    assert_one_invert_sets("circle radius 1", (0, 0), &expected, |graphics| {
        graphics.draw_circle(10, 10, 1)
    });
    assert_clear_after_2_inverts("circle radius 1", (0, 0), |graphics| {
        graphics.draw_circle(10, 10, 1)
    });
}

#[test]
fn test_a_circle_with_radius_0_leaves_a_clear_canvas_after_2_inverts() {
    assert_clear_after_2_inverts("circle radius 0", (0, 0), |graphics| {
        graphics.draw_circle(10, 10, 0)
    });
}

#[test]
fn test_circles_in_the_inverse_colour_drawn_2_times_leave_a_clear_canvas() {
    let mut numbers = Numbers(183);
    for _ in 0..200 {
        let origin = (numbers.between(-8, 8), numbers.between(-8, 8));
        let (cx, cy) = (numbers.between(-30, 90), numbers.between(-30, 80));
        let radius = numbers.between(-5, 60);
        let case = format!("circle ({cx}, {cy}) radius {radius}, origin {origin:?}");
        let draw = |graphics: &mut GraphicsSystem| graphics.draw_circle(cx, cy, radius);
        assert_clear_after_2_inverts(&case, origin, draw);
        let expected = on_canvas(&old_circle_outline(cx, cy, radius.abs()), origin);
        assert_one_invert_sets(&case, origin, &expected, draw);
    }
}

#[test]
fn test_ellipses_in_the_inverse_colour_drawn_2_times_leave_a_clear_canvas() {
    let mut numbers = Numbers(184);
    for _ in 0..200 {
        let origin = (numbers.between(-8, 8), numbers.between(-8, 8));
        let (cx, cy) = (numbers.between(-30, 90), numbers.between(-30, 80));
        let (rx, ry) = (numbers.between(-2, 60), numbers.between(-2, 60));
        let case = format!("ellipse ({cx}, {cy}) radii {rx}, {ry}, origin {origin:?}");
        let draw = |graphics: &mut GraphicsSystem| graphics.draw_ellipse(cx, cy, rx, ry);
        assert_clear_after_2_inverts(&case, origin, draw);
        let expected = on_canvas(&old_ellipse(cx, cy, rx.abs(), ry.abs()), origin);
        assert_one_invert_sets(&case, origin, &expected, draw);
    }
}

#[test]
fn test_circles_and_ellipses_in_the_store_action_set_the_old_pixels() {
    let mut numbers = Numbers(185);
    for _ in 0..200 {
        let origin = (numbers.between(-8, 8), numbers.between(-8, 8));
        let (cx, cy) = (numbers.between(-30, 90), numbers.between(-30, 80));
        let radius = numbers.between(-5, 60);
        let mut graphics = canvas(origin, 0);
        graphics.draw_circle(cx, cy, radius);
        assert_eq!(
            set_points(&graphics, origin),
            on_canvas(&old_circle_outline(cx, cy, radius.abs()), origin),
            "circle ({cx}, {cy}) radius {radius}, origin {origin:?}"
        );

        let (rx, ry) = (numbers.between(-2, 60), numbers.between(-2, 60));
        let mut graphics = canvas(origin, 0);
        graphics.draw_ellipse(cx, cy, rx, ry);
        assert_eq!(
            set_points(&graphics, origin),
            on_canvas(&old_ellipse(cx, cy, rx.abs(), ry.abs()), origin),
            "ellipse ({cx}, {cy}) radii {rx}, {ry}, origin {origin:?}"
        );
    }
}
