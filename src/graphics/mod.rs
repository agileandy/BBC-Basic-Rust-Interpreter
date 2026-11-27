//! Graphics system for BBC BASIC
//!
//! Handles display modes and graphics operations.

use std::collections::BTreeSet;
use std::fmt;

/// The largest sum of the 2 radii of an ellipse that the midpoint algorithm
/// draws step by step. Each step is 1 pixel, thus this limit keeps it fast.
const ELLIPSE_STEP_LIMIT: i64 = 1 << 16;

/// Default canvas width (1280 pixels matching BBC Micro MODE 0)
const DEFAULT_WIDTH: usize = 1280;
/// Default canvas height (1024 pixels matching BBC Micro MODE 0)
const DEFAULT_HEIGHT: usize = 1024;

/// The properties of one BBC Micro screen mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenMode {
    /// Width of the screen in pixels (0 in a text-only mode)
    pub pixel_width: usize,
    /// Height of the screen in pixels (0 in a text-only mode)
    pub pixel_height: usize,
    /// Number of logical colours
    pub colours: u8,
    /// Number of text columns
    pub text_columns: usize,
    /// Number of text rows
    pub text_rows: usize,
    /// True if PLOT, MOVE and DRAW have an effect in this mode
    pub has_graphics: bool,
}

/// The BBC Micro screen modes 0 to 7, in mode number order
const SCREEN_MODES: [ScreenMode; 8] = [
    ScreenMode::graphics(640, 256, 2, 80, 32),
    ScreenMode::graphics(320, 256, 4, 40, 32),
    ScreenMode::graphics(160, 256, 16, 20, 32),
    ScreenMode::text_only(2, 80, 25),
    ScreenMode::graphics(320, 256, 2, 40, 32),
    ScreenMode::graphics(160, 256, 4, 20, 32),
    ScreenMode::text_only(2, 40, 25),
    ScreenMode::text_only(8, 40, 25),
];

impl ScreenMode {
    const fn graphics(
        pixel_width: usize,
        pixel_height: usize,
        colours: u8,
        text_columns: usize,
        text_rows: usize,
    ) -> Self {
        Self {
            pixel_width,
            pixel_height,
            colours,
            text_columns,
            text_rows,
            has_graphics: true,
        }
    }

    const fn text_only(colours: u8, text_columns: usize, text_rows: usize) -> Self {
        Self {
            pixel_width: 0,
            pixel_height: 0,
            colours,
            text_columns,
            text_rows,
            has_graphics: false,
        }
    }

    /// The screen mode with the given number, or None if the number is not 0 to 7
    pub fn from_number(number: i32) -> Option<Self> {
        usize::try_from(number)
            .ok()
            .and_then(|index| SCREEN_MODES.get(index))
            .copied()
    }
}

/// Graphics system coordinate and state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Point {
    x: i32,
    y: i32,
}

/// Graphics canvas for drawing operations
#[derive(Debug, Clone)]
pub struct GraphicsSystem {
    /// Canvas buffer (true = pixel set, false = pixel clear)
    canvas: Vec<Vec<bool>>,
    /// Canvas width in pixels
    width: usize,
    /// Canvas height in pixels
    height: usize,
    /// Current graphics cursor position (the last point)
    current_pos: Point,
    /// The point before the last point (PLOT 80 to 87 use it for a triangle)
    previous_pos: Point,
    /// Graphics origin offset
    origin: Point,
    /// Current foreground color (0-255, though we only use on/off for now)
    foreground_color: u8,
    /// Graphics color mode (0 = set, 1 = OR, 2 = AND, 3 = XOR, 4 = invert)
    color_mode: u8,
}

impl GraphicsSystem {
    /// Create a new graphics system with default dimensions
    pub fn new() -> Self {
        Self::with_dimensions(DEFAULT_WIDTH, DEFAULT_HEIGHT)
    }

    /// Create a new graphics system with custom dimensions
    pub fn with_dimensions(width: usize, height: usize) -> Self {
        Self {
            canvas: vec![vec![false; width]; height],
            width,
            height,
            current_pos: Point { x: 0, y: 0 },
            previous_pos: Point { x: 0, y: 0 },
            origin: Point { x: 0, y: 0 },
            foreground_color: 255, // White
            color_mode: 0,         // Set mode
        }
    }

    /// Clear the graphics canvas
    pub fn clear(&mut self) {
        for row in &mut self.canvas {
            row.fill(false);
        }
    }

    /// Reset the graphics for a new screen mode (MODE): clear the canvas,
    /// move the cursor and the origin to 0, 0 and set the default colours
    pub fn reset_for_mode(&mut self, mode: &ScreenMode) {
        self.clear();
        self.current_pos = Point { x: 0, y: 0 };
        self.previous_pos = Point { x: 0, y: 0 };
        self.origin = Point { x: 0, y: 0 };
        self.foreground_color = mode.colours - 1;
        self.color_mode = 0;
    }

    /// Set graphics color mode (GCOL)
    pub fn set_color(&mut self, mode: u8, color: u8) {
        self.color_mode = mode;
        self.foreground_color = color;
    }

    /// Set graphics origin (VDU 29)
    pub fn set_origin(&mut self, x: i32, y: i32) {
        self.origin = Point { x, y };
    }

    /// Convert BBC BASIC coordinates to canvas coordinates
    fn to_canvas_coords(&self, x: i32, y: i32) -> Option<(usize, usize)> {
        // BBC BASIC uses bottom-left origin, canvas uses top-left
        // Add origin offset
        let canvas_x = x + self.origin.x;
        let canvas_y = (self.height as i32) - 1 - (y + self.origin.y);

        if canvas_x >= 0
            && canvas_x < self.width as i32
            && canvas_y >= 0
            && canvas_y < self.height as i32
        {
            Some((canvas_x as usize, canvas_y as usize))
        } else {
            None
        }
    }

    /// Set a pixel at the given coordinates
    fn set_pixel(&mut self, x: i32, y: i32) {
        if let Some((cx, cy)) = self.to_canvas_coords(x, y) {
            match self.color_mode {
                0 => self.canvas[cy][cx] = self.foreground_color > 0, // Set
                1 => self.canvas[cy][cx] |= self.foreground_color > 0, // OR
                2 => self.canvas[cy][cx] &= self.foreground_color > 0, // AND
                3 => self.canvas[cy][cx] ^= self.foreground_color > 0, // XOR
                4 => self.canvas[cy][cx] = !self.canvas[cy][cx],      // Invert
                _ => self.canvas[cy][cx] = self.foreground_color > 0,
            }
        }
    }

    /// The lowest and the highest x on the canvas, with the origin offset
    fn visible_x(&self) -> (i64, i64) {
        let min_x = -i64::from(self.origin.x);
        (min_x, min_x + self.width as i64 - 1)
    }

    /// The lowest and the highest y on the canvas, with the origin offset
    fn visible_y(&self) -> (i64, i64) {
        let min_y = -i64::from(self.origin.y);
        (min_y, min_y + self.height as i64 - 1)
    }

    /// Set a pixel if it is on the canvas. The coordinates are i64, thus a
    /// drawing function can give a point that is not an i32.
    fn set_visible_pixel(&mut self, x: i64, y: i64) {
        let (min_x, max_x) = self.visible_x();
        let (min_y, max_y) = self.visible_y();
        if (min_x..=max_x).contains(&x) && (min_y..=max_y).contains(&y) {
            if let (Ok(x), Ok(y)) = (i32::try_from(x), i32::try_from(y)) {
                self.set_pixel(x, y);
            }
        }
    }

    /// Get pixel state at given coordinates
    pub fn get_pixel(&self, x: i32, y: i32) -> Option<bool> {
        self.to_canvas_coords(x, y)
            .map(|(cx, cy)| self.canvas[cy][cx])
    }

    /// Move graphics cursor without drawing (MOVE or PLOT 4)
    pub fn move_to(&mut self, x: i32, y: i32) {
        self.set_last_point(x, y);
    }

    /// Make x, y the last point, and the old last point the point before it
    fn set_last_point(&mut self, x: i32, y: i32) {
        self.previous_pos = self.current_pos;
        self.current_pos = Point { x, y };
    }

    /// Move graphics cursor relative to current position
    pub fn move_by(&mut self, dx: i32, dy: i32) {
        self.current_pos.x += dx;
        self.current_pos.y += dy;
    }

    /// Draw a line from current position to target (DRAW or PLOT 5)
    pub fn draw_line_to(&mut self, x: i32, y: i32) {
        self.draw_line(self.current_pos.x, self.current_pos.y, x, y);
        self.set_last_point(x, y);
    }

    /// Draw a line relative to current position
    pub fn draw_line_by(&mut self, dx: i32, dy: i32) {
        let target_x = self.current_pos.x + dx;
        let target_y = self.current_pos.y + dy;
        self.draw_line_to(target_x, target_y);
    }

    /// Draw a line with the pixels of Bresenham's algorithm. Each step moves
    /// 1 pixel along the major axis. At step i, the algorithm gives the minor
    /// offset (2 * minor * i + major - 1) / (2 * major). Thus only the steps
    /// on the canvas are visited, and a long line is fast.
    fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        let (x0, y0) = (i64::from(x0), i64::from(y0));
        let (x1, y1) = (i64::from(x1), i64::from(y1));
        let (dx, dy) = ((x1 - x0).abs(), (y1 - y0).abs());
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let x_major = dx >= dy;
        let (major, minor) = if x_major { (dx, dy) } else { (dy, dx) };
        let (start, step, (low, high)) = if x_major {
            (x0, sx, self.visible_x())
        } else {
            (y0, sy, self.visible_y())
        };
        let (first, last) = if step > 0 {
            (low - start, high - start)
        } else {
            (start - high, start - low)
        };

        for i in first.max(0)..=last.min(major) {
            let offset = if major == 0 {
                0
            } else {
                let (major, minor, i) = (i128::from(major), i128::from(minor), i128::from(i));
                ((2 * minor * i + major - 1) / (2 * major)) as i64
            };
            if x_major {
                self.set_visible_pixel(x0 + sx * i, y0 + sy * offset);
            } else {
                self.set_visible_pixel(x0 + sx * offset, y0 + sy * i);
            }
        }
    }

    /// Plot with a BBC BASIC PLOT mode
    pub fn plot(&mut self, mode: u8, x: i32, y: i32) {
        // Each group of 8 modes has the same rule for bit 2: set is absolute
        // and clear is relative to the last point. The low 2 bits select the
        // action: 0 moves, 1 draws in the foreground colour, 2 and 3 draw in
        // the inverse and background colours. Those 2 colours are not
        // modelled, so the line, triangle, circle and ellipse groups only move for 2
        // and 3. The point group (64-71) sets the point for 2 and 3 also.
        //
        // These codes from 128 to 191 are not supported. They draw nothing,
        // give no error and move the last point:
        // - 128-143: flood fill
        // - 160-167: arc, 168-175: segment, 176-183: sector
        // - 184-191: move or copy a rectangle
        let absolute = (mode & 0x04) != 0;
        let (target_x, target_y) = if absolute {
            (x, y)
        } else {
            (self.current_pos.x + x, self.current_pos.y + y)
        };
        let draws = (mode & 0x03) == 1;

        match mode & 0xF8 {
            // 0-7: Move or draw a line
            0x00 if draws => {
                self.draw_line(self.current_pos.x, self.current_pos.y, target_x, target_y);
            }
            // 64-71: Plot point. Low bits 0 (64, 68) only move
            0x40 if (mode & 0x03) != 0 => {
                self.set_pixel(target_x, target_y);
            }
            // 80-87: Filled triangle with the 2 last points and the target
            0x50 if draws => {
                self.fill_triangle(
                    self.previous_pos.x,
                    self.previous_pos.y,
                    self.current_pos.x,
                    self.current_pos.y,
                    target_x,
                    target_y,
                );
            }
            // 144-151: Circle outline. 152-159: Filled circle. The centre is
            // the last point and the target is a point on the circle
            0x90 | 0x98 if draws => {
                let centre = self.current_pos;
                let dx = (i64::from(target_x) - i64::from(centre.x)) as f64;
                let dy = (i64::from(target_y) - i64::from(centre.y)) as f64;
                // The radius can be larger than an i32
                let radius = (dx * dx + dy * dy).sqrt().round() as i64;
                if mode & 0xF8 == 0x90 {
                    self.circle_outline(centre.x, centre.y, radius);
                } else {
                    self.fill_circle(centre.x, centre.y, radius);
                }
            }
            // 192-199: Ellipse outline. 200-207: Filled ellipse. The BBC
            // BASIC for Windows manual gives: "the previous two points visited
            // correspond to the centre of the ellipse and a point on a vertical
            // tangent to the ellipse (i.e. the horizontal radius). The
            // specified X,Y position lies anywhere on a horizontal tangent to
            // the ellipse (i.e. the vertical radius)."
            // (https://www.bbcbasic.co.uk/bbcwin/manual/bbcwin3.html)
            0xC0 | 0xC8 if draws => {
                let centre = self.previous_pos;
                let rx = (i64::from(self.current_pos.x) - i64::from(centre.x)).abs();
                let ry = (i64::from(target_y) - i64::from(centre.y)).abs();
                if mode & 0xF8 == 0xC0 {
                    self.ellipse_outline(centre.x, centre.y, rx, ry);
                } else {
                    self.fill_ellipse(centre.x, centre.y, rx, ry);
                }
            }
            // Default: just move cursor
            _ => {}
        }
        self.set_last_point(target_x, target_y);
    }

    /// CIRCLE x, y, r. The BBC BASIC for Windows manual gives the rule:
    /// "CIRCLE x,y,r is equivalent to MOVE x,y : PLOT 145,r,0"
    /// (https://www.bbcbasic.co.uk/bbcwin/manual/bbcwin4.html). Thus the point
    /// before the last point is the centre, and the last point is (x + r, y).
    pub fn draw_circle(&mut self, center_x: i32, center_y: i32, radius: i32) {
        self.move_to(center_x, center_y);
        self.plot(145, radius, 0);
    }

    /// Fill a circle with a horizontal line for each row. Only the rows and
    /// the columns on the canvas are visited, so a large radius is fast. The
    /// half width is an exact integer square root, thus a radius larger than
    /// an i32 does not overflow or round down.
    fn fill_circle(&mut self, center_x: i32, center_y: i32, radius: i64) {
        let (cx, cy, r) = (i64::from(center_x), i64::from(center_y), radius);
        let min_x = -i64::from(self.origin.x);
        let max_x = self.width as i64 - 1 - i64::from(self.origin.x);
        let min_y = -i64::from(self.origin.y);
        let max_y = self.height as i64 - 1 - i64::from(self.origin.y);
        for y in (cy - r).max(min_y)..=(cy + r).min(max_y) {
            let dy = y - cy;
            let half =
                (i128::from(r) * i128::from(r) - i128::from(dy) * i128::from(dy)).isqrt() as i64;
            for x in (cx - half).max(min_x)..=(cx + half).min(max_x) {
                self.set_pixel(x as i32, y as i32);
            }
        }
    }

    /// Draw a circle outline with the pixels of the midpoint circle
    /// algorithm. At octant row y, the algorithm gives the largest x with
    /// x * (x - 1) <= r * (r - 1) + 1 - y * y, and it stops when x < y. Only
    /// the rows that can put a point on the canvas are visited, each 1 time.
    fn circle_outline(&mut self, center_x: i32, center_y: i32, radius: i64) {
        if radius <= 0 {
            return;
        }

        let (cx, cy, r) = (i64::from(center_x), i64::from(center_y), i128::from(radius));
        let (min_x, max_x) = self.visible_x();
        let (min_y, max_y) = self.visible_y();
        // The rows y that put a point at row cy + y, row cy - y, column
        // cx + y or column cx - y on the canvas
        let mut row_ranges = [
            (min_y - cy, max_y - cy),
            (cy - max_y, cy - min_y),
            (min_x - cx, max_x - cx),
            (cx - max_x, cx - min_x),
        ];
        row_ranges.sort_unstable();

        let mut next_row = 0;
        for (first, last) in row_ranges {
            for y in first.max(next_row)..=last {
                let limit = r * (r - 1) + 1 - i128::from(y) * i128::from(y);
                if limit < 0 {
                    return;
                }
                let x = ((limit * 4 + 1).isqrt() + 1) as i64 / 2;
                if x < y {
                    return;
                }

                // Draw 8 octants
                self.set_visible_pixel(cx + x, cy + y);
                self.set_visible_pixel(cx + y, cy + x);
                self.set_visible_pixel(cx - y, cy + x);
                self.set_visible_pixel(cx - x, cy + y);
                self.set_visible_pixel(cx - x, cy - y);
                self.set_visible_pixel(cx - y, cy - x);
                self.set_visible_pixel(cx + y, cy - x);
                self.set_visible_pixel(cx + x, cy - y);
            }
            next_row = next_row.max(last + 1);
        }
    }

    /// ELLIPSE x, y, a, b. The BBC BASIC for Windows manual gives the rule:
    /// "ELLIPSE x,y,a,b is equivalent to MOVE x,y : PLOT 0,a,0 : PLOT 193,0,b"
    /// (https://www.bbcbasic.co.uk/bbcwin/manual/bbcwin4.html). Thus the point
    /// before the last point is (x + a, y), and the last point is (x + a, y + b).
    pub fn draw_ellipse(&mut self, center_x: i32, center_y: i32, rx: i32, ry: i32) {
        self.move_to(center_x, center_y);
        self.plot(0, rx, 0);
        self.plot(193, 0, ry);
    }

    /// Fill an ellipse with a horizontal line for each row. Only the rows and
    /// the columns on the canvas are visited, so a large radius is fast.
    fn fill_ellipse(&mut self, center_x: i32, center_y: i32, rx: i64, ry: i64) {
        if rx <= 0 || ry <= 0 {
            return;
        }
        let (cx, cy) = (i64::from(center_x), i64::from(center_y));
        let min_x = -i64::from(self.origin.x);
        let max_x = self.width as i64 - 1 - i64::from(self.origin.x);
        let min_y = -i64::from(self.origin.y);
        let max_y = self.height as i64 - 1 - i64::from(self.origin.y);
        for y in (cy - ry).max(min_y)..=(cy + ry).min(max_y) {
            let dy = (y - cy) as f64 / ry as f64;
            let half = (rx as f64 * (1.0 - dy * dy).sqrt()) as i64;
            for x in (cx - half).max(min_x)..=(cx + half).min(max_x) {
                self.set_pixel(x as i32, y as i32);
            }
        }
    }

    /// Draw an ellipse outline using midpoint ellipse algorithm. The terms
    /// are i128, so a large radius does not overflow. The algorithm visits
    /// each step from the top to the side. If the radii are too large for
    /// that, only the rows and the columns on the canvas are visited.
    fn ellipse_outline(&mut self, center_x: i32, center_y: i32, rx: i64, ry: i64) {
        if rx <= 0 || ry <= 0 {
            return;
        }

        let (cx, cy) = (i64::from(center_x), i64::from(center_y));
        if rx + ry > ELLIPSE_STEP_LIMIT {
            self.large_ellipse_outline(cx, cy, rx, ry);
            return;
        }
        let (rx, ry) = (i128::from(rx), i128::from(ry));

        // Region 1
        let mut x = 0;
        let mut y = ry;
        let rx_sq = rx * rx;
        let ry_sq = ry * ry;
        let two_rx_sq = 2 * rx_sq;
        let two_ry_sq = 2 * ry_sq;

        let mut px = 0;
        let mut py = two_rx_sq * y;

        // Plot initial points
        self.plot_ellipse_points(cx, cy, x, y);

        // Region 1
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
            self.plot_ellipse_points(cx, cy, x, y);
        }

        // Region 2
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
            self.plot_ellipse_points(cx, cy, x, y);
        }
    }

    /// Helper to plot 4 quadrants of ellipse. A point that is not on the
    /// canvas is not set.
    fn plot_ellipse_points(&mut self, cx: i64, cy: i64, x: i128, y: i128) {
        let (x, y) = (x as i64, y as i64);
        self.set_visible_pixel(cx + x, cy + y);
        self.set_visible_pixel(cx - x, cy + y);
        self.set_visible_pixel(cx + x, cy - y);
        self.set_visible_pixel(cx - x, cy - y);
    }

    /// Draw the outline of an ellipse with radii above ELLIPSE_STEP_LIMIT.
    /// Visit only the rows and the columns on the canvas, and set the 2 edge
    /// points of each. Each point is set 1 time.
    fn large_ellipse_outline(&mut self, cx: i64, cy: i64, rx: i64, ry: i64) {
        let (min_x, max_x) = self.visible_x();
        let (min_y, max_y) = self.visible_y();
        // The half width of the ellipse at the offset from the centre
        let half = |radius: i64, other_radius: i64, offset: i64| {
            let part = offset as f64 / other_radius as f64;
            (radius as f64 * (1.0 - part * part).max(0.0).sqrt()).round() as i64
        };

        let mut points = BTreeSet::new();
        for y in (cy - ry).max(min_y)..=(cy + ry).min(max_y) {
            let x = half(rx, ry, y - cy);
            points.insert((cx - x, y));
            points.insert((cx + x, y));
        }
        for x in (cx - rx).max(min_x)..=(cx + rx).min(max_x) {
            let y = half(ry, rx, x - cx);
            points.insert((x, cy - y));
            points.insert((x, cy + y));
        }
        for (x, y) in points {
            self.set_visible_pixel(x, y);
        }
    }

    /// Draw a filled rectangle. Only the rows and the columns on the canvas
    /// are visited, so a large rectangle is fast.
    pub fn draw_rectangle(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, filled: bool) {
        let min_x = i64::from(x1.min(x2));
        let max_x = i64::from(x1.max(x2));
        let min_y = i64::from(y1.min(y2));
        let max_y = i64::from(y1.max(y2));
        let (visible_min_x, visible_max_x) = self.visible_x();
        let (visible_min_y, visible_max_y) = self.visible_y();
        let columns = min_x.max(visible_min_x)..=max_x.min(visible_max_x);
        let rows = min_y.max(visible_min_y)..=max_y.min(visible_max_y);

        if filled {
            // Fill the rectangle
            for y in rows {
                for x in columns.clone() {
                    self.set_visible_pixel(x, y);
                }
            }
        } else {
            // Draw outline
            for x in columns {
                self.set_visible_pixel(x, min_y);
                self.set_visible_pixel(x, max_y);
            }
            for y in rows {
                self.set_visible_pixel(min_x, y);
                self.set_visible_pixel(max_x, y);
            }
        }

        // Update current position
        self.current_pos = Point { x: x2, y: y2 };
    }

    /// Draw a triangle
    #[allow(clippy::too_many_arguments)]
    pub fn draw_triangle(
        &mut self,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        x3: i32,
        y3: i32,
        filled: bool,
    ) {
        if filled {
            // Filled triangle using scanline algorithm
            self.fill_triangle(x1, y1, x2, y2, x3, y3);
        } else {
            // Outline triangle - draw three lines
            self.draw_line(x1, y1, x2, y2);
            self.draw_line(x2, y2, x3, y3);
            self.draw_line(x3, y3, x1, y1);
        }

        // Update current position to last vertex
        self.current_pos = Point { x: x3, y: y3 };
    }

    /// Fill a triangle using scanline algorithm. Only the rows and the
    /// columns on the canvas are visited, and the products are i128, so
    /// large coordinates are fast and do not overflow.
    fn fill_triangle(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, x3: i32, y3: i32) {
        // Sort vertices by y-coordinate
        let mut verts = [(x1, y1), (x2, y2), (x3, y3)].map(|(x, y)| (i64::from(x), i64::from(y)));
        verts.sort_by_key(|v| v.1);
        let (x1, y1) = verts[0];
        let (x2, y2) = verts[1];
        let (x3, y3) = verts[2];
        // The x on the edge from (xa, ya) to (xb, yb) at row y
        let edge_x = |xa: i64, ya: i64, xb: i64, yb: i64, y: i64| {
            xa + (i128::from(xb - xa) * i128::from(y - ya) / i128::from(yb - ya)) as i64
        };
        let (min_x, max_x) = self.visible_x();
        let (min_y, max_y) = self.visible_y();

        // Fill scanlines
        for y in y1.max(min_y)..=y3.min(max_y) {
            let mut x_left = x1;
            let mut x_right = x1;

            // Calculate x intersections with triangle edges
            if y3 == y1 {
                // All 3 corners are on one row
                x_left = x1.min(x2).min(x3);
                x_right = x1.max(x2).max(x3);
            } else if y <= y2 {
                // Upper part of triangle. If 2 corners are on the first
                // row, that row is filled from one corner to the other.
                x_left = if y2 != y1 {
                    edge_x(x1, y1, x2, y2, y)
                } else {
                    x2
                };
                if y3 != y1 {
                    x_right = edge_x(x1, y1, x3, y3, y);
                }
            } else {
                // Lower part of triangle
                if y3 != y2 {
                    x_left = edge_x(x2, y2, x3, y3, y);
                }
                if y3 != y1 {
                    x_right = edge_x(x1, y1, x3, y3, y);
                }
            }

            // Ensure x_left <= x_right
            if x_left > x_right {
                std::mem::swap(&mut x_left, &mut x_right);
            }

            // Draw horizontal line
            for x in x_left.max(min_x)..=x_right.min(max_x) {
                self.set_visible_pixel(x, y);
            }
        }
    }

    /// Flood fill starting from a point
    pub fn flood_fill(&mut self, start_x: i32, start_y: i32) {
        // Get the target color to replace
        let target_color = match self.get_pixel(start_x, start_y) {
            Some(color) => color,
            None => return, // Outside bounds
        };

        let fill_color = self.foreground_color > 0;

        // Don't fill if already the target color
        if target_color == fill_color {
            return;
        }

        // Stack-based flood fill (avoids recursion stack overflow)
        let mut stack = Vec::new();
        stack.push((start_x, start_y));

        while let Some((x, y)) = stack.pop() {
            // Check if pixel is valid and matches target color
            if let Some(color) = self.get_pixel(x, y) {
                if color == target_color {
                    self.set_pixel(x, y);

                    // Add adjacent pixels to stack
                    stack.push((x + 1, y));
                    stack.push((x - 1, y));
                    stack.push((x, y + 1));
                    stack.push((x, y - 1));
                }
            }
        }
    }

    /// Get current graphics cursor position
    pub fn get_position(&self) -> (i32, i32) {
        (self.current_pos.x, self.current_pos.y)
    }

    /// Render the canvas to a string (ASCII art representation)
    pub fn render(&self) -> String {
        self.render_scaled(4, 8)
    }

    /// Render the canvas with scaling (for terminal display)
    /// scale_x: how many pixels per character horizontally
    /// scale_y: how many pixels per character vertically
    pub fn render_scaled(&self, scale_x: usize, scale_y: usize) -> String {
        let mut output = String::new();
        let chars_wide = self.width / scale_x;
        let chars_high = self.height / scale_y;

        // Top border
        output.push('+');
        output.push_str(&"-".repeat(chars_wide));
        output.push_str("+\n");

        // Canvas content
        for row_block in 0..chars_high {
            output.push('|');
            for col_block in 0..chars_wide {
                // Sample the block and count set pixels
                let mut pixel_count = 0;
                let mut total_pixels = 0;

                for dy in 0..scale_y {
                    let y = row_block * scale_y + dy;
                    if y >= self.height {
                        break;
                    }
                    for dx in 0..scale_x {
                        let x = col_block * scale_x + dx;
                        if x >= self.width {
                            break;
                        }
                        if self.canvas[y][x] {
                            pixel_count += 1;
                        }
                        total_pixels += 1;
                    }
                }

                // Choose character based on pixel density
                let density = if total_pixels > 0 {
                    pixel_count * 4 / total_pixels
                } else {
                    0
                };

                let ch = match density {
                    0 => ' ',
                    1 => '░',
                    2 => '▒',
                    3 => '▓',
                    _ => '█',
                };
                output.push(ch);
            }
            output.push_str("|\n");
        }

        // Bottom border
        output.push('+');
        output.push_str(&"-".repeat(chars_wide));
        output.push('+');

        output
    }
}

impl Default for GraphicsSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for GraphicsSystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.render())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_graphics_system() {
        let gfx = GraphicsSystem::new();
        assert_eq!(gfx.width, DEFAULT_WIDTH);
        assert_eq!(gfx.height, DEFAULT_HEIGHT);
    }

    #[test]
    fn test_move_to() {
        let mut gfx = GraphicsSystem::new();
        gfx.move_to(100, 200);
        assert_eq!(gfx.get_position(), (100, 200));
    }

    #[test]
    fn test_draw_line() {
        let mut gfx = GraphicsSystem::with_dimensions(100, 100);
        gfx.move_to(10, 10);
        gfx.draw_line_to(20, 20);
        // Verify line drawn
        assert!(gfx.get_pixel(10, 10).unwrap());
        assert!(gfx.get_pixel(20, 20).unwrap());
    }

    #[test]
    fn test_clear() {
        let mut gfx = GraphicsSystem::with_dimensions(100, 100);
        gfx.set_pixel(50, 50);
        assert!(gfx.get_pixel(50, 50).unwrap());
        gfx.clear();
        assert!(!gfx.get_pixel(50, 50).unwrap());
    }

    #[test]
    fn test_circle() {
        let mut gfx = GraphicsSystem::with_dimensions(200, 200);
        gfx.draw_circle(100, 100, 50);
        // Verify some points on the circle
        assert!(gfx.get_pixel(150, 100).unwrap()); // Right point
        assert!(gfx.get_pixel(50, 100).unwrap()); // Left point
    }
}
