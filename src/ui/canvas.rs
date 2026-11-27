//! The graphics canvas of the RUN pane (issue #173).
//!
//! The worker thread scales the canvas down to the samples of the picture
//! area: 2 samples for each character cell, one above the other. Each
//! sample is the highest colour number in its area, so that a thin line
//! stays visible. The pane draws each cell with the upper half block: the
//! foreground is the top sample and the background is the bottom sample.
//!
//! The picture area keeps the aspect 4 to 3 of a BBC screen, as large as
//! the pane permits, in the centre of the pane. A character cell is about 2
//! times as high as it is wide. The colours come from the BBC palette of
//! the mode in `src/ui/theme.rs`.

use super::theme::palette;
use crate::graphics::{GraphicsSystem, ScreenMode};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};

/// The upper half block.
const HALF: &str = "\u{2580}";

/// The canvas scaled to the samples of a picture area.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Picture {
    /// The count of sample columns: the width of the area in cells
    width: usize,
    /// The count of sample rows: 2 times the height of the area in cells
    height: usize,
    /// The colour count of the screen mode
    colours: u8,
    /// The colour numbers, row by row from the top
    samples: Vec<u8>,
}

impl Picture {
    /// The picture of the canvas for an area of `columns` by `rows` cells.
    /// None if the screen mode has no graphics.
    pub(crate) fn of(
        graphics: &GraphicsSystem,
        mode: ScreenMode,
        columns: usize,
        rows: usize,
    ) -> Option<Self> {
        if !mode.has_graphics {
            return None;
        }
        let (width, height) = graphics.canvas_size();
        let samples = scale(width, height, |y| graphics.canvas_row(y), columns, 2 * rows);
        Some(Self {
            width: columns,
            height: 2 * rows,
            colours: mode.colours,
            samples,
        })
    }

    /// The samples for an area of `columns` by `rows` cells. If the size is
    /// not the size of the picture, scale the samples. The worker sends a
    /// new picture after a resize, and this one shows until it comes.
    fn samples_for(&self, columns: usize, rows: usize) -> std::borrow::Cow<'_, [u8]> {
        if (columns, 2 * rows) == (self.width, self.height) {
            return std::borrow::Cow::Borrowed(&self.samples);
        }
        let width = self.width;
        let row = |y: usize| &self.samples[y * width..(y + 1) * width];
        std::borrow::Cow::Owned(scale(width, self.height, row, columns, 2 * rows))
    }
}

/// Scale a grid of colour numbers of `width` by `height`, which `row` gives
/// row by row, to a grid of `columns` by `rows`. Each new value is the
/// highest colour number in its area of the old grid. If the new grid is
/// larger, an area is one old value.
pub(crate) fn scale<'a>(
    width: usize,
    height: usize,
    row: impl Fn(usize) -> &'a [u8],
    columns: usize,
    rows: usize,
) -> Vec<u8> {
    let mut out = vec![0; columns * rows];
    if width == 0 || height == 0 {
        return out;
    }
    // The old range of new index i of n: never empty, never past the end
    let span = |i: usize, n: usize, old: usize| {
        let start = i * old / n;
        start..((i + 1) * old / n).clamp(start + 1, old)
    };
    let spans: Vec<_> = (0..columns).map(|x| span(x, columns, width)).collect();
    for y in 0..rows {
        let line = &mut out[y * columns..(y + 1) * columns];
        for old_y in span(y, rows, height) {
            let old_row = row(old_y);
            for (value, columns) in line.iter_mut().zip(&spans) {
                let highest = old_row
                    .get(columns.clone())
                    .and_then(|area| area.iter().max())
                    .copied()
                    .unwrap_or(0);
                *value = (*value).max(highest);
            }
        }
    }
    out
}

/// The picture area in a pane: the aspect 4 to 3, as large as the pane
/// permits, in the centre. The width in cells is 8 / 3 of the height,
/// because a cell is 2 times as high as it is wide.
pub(crate) fn picture_area(area: Rect) -> Rect {
    let height = u32::from(area.height).min(u32::from(area.width) * 3 / 8);
    let width = (height * 8 / 3).min(u32::from(area.width));
    // Both are not more than the pane, so they fit in u16
    let (width, height) = (width as u16, height as u16);
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}

/// The picture area for the text screen `text` of a mode with `columns` by
/// `rows` cells, in the pane `area` (issue #200). If the pane shows all of
/// the text screen, and the aspect of the text screen is from 0.8 to 1.25
/// times 4 to 3, the picture takes the text screen. Then a text cell and
/// the graphics at the same place of a BBC screen are together. If not, the
/// picture keeps the area of `picture_area`, so that it stays readable.
pub(crate) fn text_picture_area(area: Rect, text: Rect, columns: usize, rows: usize) -> Rect {
    let whole = usize::from(text.width) == columns && usize::from(text.height) == rows;
    // The aspect over 4 to 3 is 3 * width / (8 * height), because a cell is
    // 2 times as high as it is wide
    let (width, height) = (u32::from(text.width), u32::from(text.height));
    let near = 15 * width >= 32 * height && 3 * width <= 10 * height;
    if whole && near {
        text
    } else {
        picture_area(area)
    }
}

/// Draw the picture in the area. A cell of the text screen `text` that is
/// not a space keeps its text: the text of a BBC Micro shows above the
/// graphics. Outside the text screen, the picture covers the border.
pub(crate) fn draw(buffer: &mut Buffer, area: Rect, picture: &Picture, text: Rect) {
    let (columns, rows) = (usize::from(area.width), usize::from(area.height));
    let samples = picture.samples_for(columns, rows);
    for y in 0..rows {
        for x in 0..columns {
            let top = samples[2 * y * columns + x];
            let bottom = samples[(2 * y + 1) * columns + x];
            if top == 0 && bottom == 0 {
                continue;
            }
            let position = Position::new(area.x + x as u16, area.y + y as u16);
            if let Some(cell) = buffer.cell_mut(position) {
                if cell.symbol() == " " || !text.contains(position) {
                    cell.set_symbol(HALF)
                        .set_fg(palette(top, picture.colours))
                        .set_bg(palette(bottom, picture.colours));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows<'a>(grid: &'a [u8], width: usize) -> impl Fn(usize) -> &'a [u8] {
        move |y| &grid[y * width..(y + 1) * width]
    }

    #[test]
    fn a_smaller_grid_keeps_the_highest_colour_of_each_area() {
        // A canvas of 8 by 4, with 3 pixels set
        let mut canvas = vec![0; 8 * 4];
        canvas[1] = 1; // x 1, y 0
        canvas[3 * 8 + 6] = 2; // x 6, y 3
        canvas[3 * 8 + 7] = 1; // x 7, y 3
        let grid = scale(8, 4, rows(&canvas, 8), 4, 2);
        assert_eq!(grid, [1, 0, 0, 0, 0, 0, 0, 2]);
    }

    #[test]
    fn a_larger_grid_repeats_each_value() {
        let grid = scale(2, 2, rows(&[1, 2, 3, 0], 2), 4, 4);
        assert_eq!(grid, [1, 1, 2, 2, 1, 1, 2, 2, 3, 3, 0, 0, 3, 3, 0, 0]);
    }

    #[test]
    fn a_size_that_does_not_divide_covers_each_old_value() {
        // 5 old columns to 3 new columns: 0..1, 1..3, 3..5
        let grid = scale(5, 1, rows(&[1, 0, 2, 0, 3], 5), 3, 1);
        assert_eq!(grid, [1, 2, 3]);
    }

    #[test]
    fn the_canvas_of_a_mode_scales_to_the_cells_of_the_area() {
        let mut graphics = GraphicsSystem::with_dimensions(1280, 1024);
        let mode = ScreenMode::from_number(1).expect("mode 1");
        graphics.reset_for_mode(&mode);
        graphics.draw_rectangle(0, 0, 639, 511, true);
        let picture = Picture::of(&graphics, mode, 4, 2).expect("a picture");
        // 4 by 4 samples: the bottom left quarter is colour 3
        assert_eq!(
            picture.samples,
            [0, 0, 0, 0, 0, 0, 0, 0, 3, 3, 0, 0, 3, 3, 0, 0]
        );
        let text_only = ScreenMode::from_number(3).expect("mode 3");
        assert_eq!(Picture::of(&graphics, text_only, 4, 2), None);
    }

    #[test]
    fn the_picture_area_has_the_aspect_4_to_3_in_the_centre() {
        assert_eq!(
            picture_area(Rect::new(0, 1, 100, 32)),
            Rect::new(7, 1, 85, 32)
        );
        // A wide pane: the height limits the area. A narrow pane: the width
        assert_eq!(
            picture_area(Rect::new(0, 0, 60, 18)),
            Rect::new(6, 0, 48, 18)
        );
        assert_eq!(
            picture_area(Rect::new(0, 0, 16, 20)),
            Rect::new(0, 7, 16, 6)
        );
        assert_eq!(picture_area(Rect::new(0, 0, 0, 0)), Rect::new(0, 0, 0, 0));
    }

    #[test]
    fn the_picture_takes_the_text_screen_only_if_it_shows_all_of_it_near_4_to_3() {
        // Mode 0 (80 by 32) in a pane of 120 by 38: the text screen
        let pane = Rect::new(0, 1, 120, 38);
        let text = Rect::new(20, 1, 80, 32);
        assert_eq!(text_picture_area(pane, text, 80, 32), text);
        // Mode 0 in a pane of 80 by 22: only 22 of 32 rows show
        let small = Rect::new(0, 1, 80, 22);
        assert_eq!(
            text_picture_area(small, Rect::new(0, 1, 80, 22), 80, 32),
            picture_area(small)
        );
        // Mode 1 (40 by 32): the aspect is 0.47 times 4 to 3
        assert_eq!(
            text_picture_area(pane, Rect::new(40, 1, 40, 32), 40, 32),
            picture_area(pane)
        );
    }
}
