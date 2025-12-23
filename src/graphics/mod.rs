//! Graphics system for BBC BASIC
//!
//! Handles display modes and graphics operations.

use std::fmt;

/// Default canvas width (1280 pixels matching BBC Micro MODE 0)
const DEFAULT_WIDTH: usize = 1280;
/// Default canvas height (1024 pixels matching BBC Micro MODE 0)
const DEFAULT_HEIGHT: usize = 1024;

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
    /// Current graphics cursor position
    current_pos: Point,
    /// Graphics origin offset
    origin: Point,
    /// Current foreground color (0-255, though we only use on/off for now)
    foreground_color: u8,
    /// Current background color
    background_color: u8,
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
            origin: Point { x: 0, y: 0 },
            foreground_color: 255, // White
            background_color: 0,   // Black
            color_mode: 0,         // Set mode
        }
    }

    /// Clear the graphics canvas
    pub fn clear(&mut self) {
        for row in &mut self.canvas {
            row.fill(false);
        }
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
                4 => self.canvas[cy][cx] = !self.canvas[cy][cx],       // Invert
                _ => self.canvas[cy][cx] = self.foreground_color > 0,
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
        self.current_pos = Point { x, y };
    }

    /// Draw a line relative to current position
    pub fn draw_line_by(&mut self, dx: i32, dy: i32) {
        let target_x = self.current_pos.x + dx;
        let target_y = self.current_pos.y + dy;
        self.draw_line_to(target_x, target_y);
    }

    /// Draw a line using Bresenham's algorithm
    fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx - dy;
        let mut x = x0;
        let mut y = y0;

        loop {
            self.set_pixel(x, y);

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
    }

    /// Plot a point with specified plot mode
    pub fn plot(&mut self, mode: u8, x: i32, y: i32) {
        // BBC BASIC plot modes:
        // 0-3: Move cursor (relative to different points)
        // 4-7: Line drawing
        // 64-127: Point plotting
        // 128-191: Triangles
        // 192-255: Circles/arcs

        let relative = (mode & 0x04) != 0; // Bit 2 determines relative mode
        let (target_x, target_y) = if relative {
            (self.current_pos.x + x, self.current_pos.y + y)
        } else {
            (x, y)
        };

        match mode & 0xF8 {
            // 0-7: Move/line operations
            0x00 => {
                // Move operations (modes 0-3)
                if (mode & 0x04) == 0 {
                    self.move_to(x, y);
                } else {
                    self.move_by(x, y);
                }
            }
            // 4-7: Draw line
            _ if mode >= 4 && mode <= 7 => {
                if relative {
                    self.draw_line_by(x, y);
                } else {
                    self.draw_line_to(target_x, target_y);
                }
            }
            // 64-71: Plot point
            _ if mode >= 64 && mode <= 71 => {
                self.set_pixel(target_x, target_y);
                self.current_pos = Point {
                    x: target_x,
                    y: target_y,
                };
            }
            // Default: just move cursor
            _ => {
                self.current_pos = Point {
                    x: target_x,
                    y: target_y,
                };
            }
        }
    }

    /// Draw a circle using midpoint circle algorithm
    pub fn draw_circle(&mut self, center_x: i32, center_y: i32, radius: i32) {
        if radius <= 0 {
            return;
        }

        let mut x = radius;
        let mut y = 0;
        let mut err = 0;

        while x >= y {
            // Draw 8 octants
            self.set_pixel(center_x + x, center_y + y);
            self.set_pixel(center_x + y, center_y + x);
            self.set_pixel(center_x - y, center_y + x);
            self.set_pixel(center_x - x, center_y + y);
            self.set_pixel(center_x - x, center_y - y);
            self.set_pixel(center_x - y, center_y - x);
            self.set_pixel(center_x + y, center_y - x);
            self.set_pixel(center_x + x, center_y - y);

            y += 1;
            if err <= 0 {
                err += 2 * y + 1;
            } else {
                x -= 1;
                err += 2 * (y - x) + 1;
            }
        }

        // Update current position to circle center
        self.current_pos = Point {
            x: center_x,
            y: center_y,
        };
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
