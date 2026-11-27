//! The EDIT pane: a full-screen editor for the program text.
//!
//! The text is in the `Document` of the `Context`. This pane holds only the
//! view state: the cursor, the scroll position and the marks of the faults.
//! The line number of each line shows in its own dim column, but it stays
//! part of the line text.

use super::{Context, Document, LineCache, LineMark, Outcome};
use crate::program::ProgramStore;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use std::cell::Cell;

/// The smallest width of the line number column.
const NUMBER_WIDTH: usize = 5;
/// The text between the line number column and the statements.
const BAR: &str = " │ ";
/// The text for a typed tab character.
const TAB_TEXT: &str = "  ";
/// The page for Page Up and Page Down before the first draw.
const DEFAULT_PAGE: usize = 20;

/// The state of the EDIT pane.
#[derive(Debug, Default)]
pub struct EditorPane {
    /// The cursor: the line index and the character index in the line
    row: usize,
    col: usize,
    /// The first line and the first statement column on the screen. The draw
    /// changes them, so that the cursor is on the screen.
    top: Cell<usize>,
    left: Cell<usize>,
    /// The height of the pane at the last draw
    page: Cell<usize>,
    /// The faults of the text at the last check
    marks: Vec<LineMark>,
    /// The check results of the line texts at the last check
    cache: LineCache,
    /// The size of the program in bytes at the last check
    size: usize,
}

impl EditorPane {
    /// A pane for a document, with the marks of its text.
    pub fn new(document: &Document) -> Self {
        let mut pane = Self::default();
        pane.check(document);
        pane
    }

    /// The cursor: the line index and the character index, from 0.
    pub fn cursor(&self) -> (usize, usize) {
        (self.row, self.col)
    }

    pub fn marks(&self) -> &[LineMark] {
        &self.marks
    }

    /// The text for the status line: the line, the column, the count of
    /// lines and the size of the program in bytes.
    pub fn status(&self, document: &Document) -> String {
        format!(
            "Ln {} Col {} │ {} lines │ {} bytes",
            self.row + 1,
            self.col + 1,
            document.lines().len(),
            self.size
        )
    }

    /// Make the program and the marks again from the text.
    fn check(&mut self, document: &Document) -> ProgramStore {
        let (program, marks) = document.to_program_cached(&mut self.cache);
        self.marks = marks;
        self.size = program.size_in_bytes();
        program
    }

    /// The user leaves the edit mode: the text becomes the stored program.
    pub fn leave(&mut self, ctx: &mut Context) {
        ctx.program = self.check(&ctx.document);
    }

    /// The user comes back to the edit mode. Another mode can replace the
    /// text, so keep the cursor in the text and make the marks again.
    pub fn resume(&mut self, ctx: &Context) {
        let doc = &ctx.document;
        self.move_to(doc, self.row.min(doc.lines().len() - 1), self.col);
        self.check(doc);
    }

    /// Put the cursor at the start of a line, such as the line of a fault
    /// or of a run error (issue #171).
    pub fn go_to_line(&mut self, doc: &Document, row: usize) {
        self.move_to(doc, row.min(doc.lines().len() - 1), 0);
    }

    pub fn key(&mut self, key: KeyEvent, ctx: &mut Context) -> Outcome {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let doc = &mut ctx.document;
        let last = doc.lines().len() - 1;
        let page = match self.page.get() {
            0 => DEFAULT_PAGE,
            page => page,
        };
        match key.code {
            KeyCode::Left if self.col > 0 => self.col -= 1,
            KeyCode::Left if self.row > 0 => self.move_to(doc, self.row - 1, usize::MAX),
            KeyCode::Right if self.col < doc.line_len(self.row) => self.col += 1,
            KeyCode::Right if self.row < last => self.move_to(doc, self.row + 1, 0),
            KeyCode::Left | KeyCode::Right => {}
            KeyCode::Up => self.move_to(doc, self.row.saturating_sub(1), self.col),
            KeyCode::Down => self.move_to(doc, (self.row + 1).min(last), self.col),
            KeyCode::PageUp => self.move_to(doc, self.row.saturating_sub(page), self.col),
            KeyCode::PageDown => self.move_to(doc, (self.row + page).min(last), self.col),
            KeyCode::Home if ctrl => self.move_to(doc, 0, 0),
            KeyCode::End if ctrl => self.move_to(doc, last, usize::MAX),
            KeyCode::Home => self.col = 0,
            KeyCode::End => self.col = doc.line_len(self.row),
            KeyCode::Enter => {
                self.enter(doc);
                self.check(doc);
            }
            KeyCode::Backspace => {
                self.backspace(doc);
                self.check(doc);
            }
            KeyCode::Delete => {
                self.delete(doc);
                self.check(doc);
            }
            KeyCode::Char('n' | 'N') if ctrl => self.renumber(ctx),
            KeyCode::Char(c) if !ctrl && !alt => {
                self.insert(doc, &typed_text(c));
                self.check(doc);
            }
            _ => return Outcome::Ignored,
        }
        Outcome::Used
    }

    /// Put pasted text at the cursor. A newline starts a new line with no
    /// line number, a tab becomes 2 spaces, and a carriage return and other
    /// control characters are left out.
    pub fn paste(&mut self, text: &str, ctx: &mut Context) {
        let doc = &mut ctx.document;
        for (index, part) in text.split('\n').enumerate() {
            if index > 0 {
                doc.split(self.row, self.col, "");
                self.row += 1;
                self.col = 0;
            }
            let part: String = part
                .chars()
                .filter(|&c| c == '\t' || !c.is_control())
                .collect::<String>()
                .replace('\t', TAB_TEXT);
            self.insert(doc, &part);
        }
        self.check(doc);
    }

    /// Move the cursor to a line, with the column in the line.
    fn move_to(&mut self, doc: &Document, row: usize, col: usize) {
        self.row = row;
        self.col = col.min(doc.line_len(row));
    }

    fn insert(&mut self, doc: &mut Document, text: &str) {
        if text.is_empty() {
            return;
        }
        doc.insert(self.row, self.col, text);
        self.col += text.chars().count();
    }

    /// Move the text after the cursor to a new line with the next free line
    /// number. At the start of a line that is not empty, put an empty line
    /// above it.
    fn enter(&mut self, doc: &mut Document) {
        let prefix = if self.col == 0 && doc.line_len(self.row) > 0 {
            String::new()
        } else {
            doc.next_line_number(self.row)
                .map_or(String::new(), |n| format!("{} ", n))
        };
        doc.split(self.row, self.col, &prefix);
        self.row += 1;
        self.col = prefix.len();
    }

    fn backspace(&mut self, doc: &mut Document) {
        if self.col > 0 {
            self.col -= 1;
            doc.remove(self.row, self.col);
        } else if self.row > 0 {
            self.row -= 1;
            self.col = doc.line_len(self.row);
            doc.join(self.row);
        }
    }

    fn delete(&mut self, doc: &mut Document) {
        if self.col < doc.line_len(self.row) {
            doc.remove(self.row, self.col);
        } else if self.row + 1 < doc.lines().len() {
            doc.join(self.row);
        }
    }

    /// Renumber the program with the function of RENUMBER, and write the
    /// listed form of each line back. A text with a mark stays unchanged.
    fn renumber(&mut self, ctx: &mut Context) {
        let (mut program, marks) = ctx.document.to_program();
        if !marks.is_empty() {
            ctx.message = "Correct the marked lines first".to_string();
            return;
        }
        match program.renumber(10, 10) {
            Ok(failed) => {
                ctx.document.set_program(&program);
                let failed: Vec<String> = failed.iter().map(u16::to_string).collect();
                ctx.message = if failed.is_empty() {
                    String::new()
                } else {
                    format!("Failed at {}", failed.join(", "))
                };
            }
            Err(e) => ctx.message = e,
        }
        self.resume(ctx);
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, ctx: &Context) {
        let plain = Style::new().fg(Color::White).bg(Color::Black);
        let dim = Style::new().fg(Color::DarkGray).bg(Color::Black);
        let red = Style::new().fg(Color::Red).bg(Color::Black);
        frame.render_widget(Paragraph::new("").style(plain), area);
        let lines = ctx.document.lines();
        let height = usize::from(area.height).max(1);
        self.page.set(height);

        // The first line on the screen
        let mut top = self.top.get().min(self.row);
        if self.row >= top + height {
            top = self.row + 1 - height;
        }
        self.top.set(top);
        let visible = &lines[top..lines.len().min(top + height)];
        let number_width = visible
            .iter()
            .map(|line| split_number(line).0.len())
            .max()
            .unwrap_or(0)
            .max(NUMBER_WIDTH);
        let text_width = usize::from(area.width)
            .saturating_sub(number_width + BAR.chars().count())
            .max(1);

        // The first statement column on the screen, and the cursor
        let (number, sep, _) = split_number(&lines[self.row]);
        let digits = number.len();
        let in_number = self.col < digits || (self.col == digits && sep > 0);
        let (left, cursor_x) = if in_number {
            (0, number_width - digits + self.col)
        } else {
            let column = self.col - digits - sep;
            let mut left = self.left.get().min(column);
            if column >= left + text_width {
                left = column + 1 - text_width;
            }
            (left, number_width + BAR.chars().count() + column - left)
        };
        self.left.set(left);

        for (offset, line) in visible.iter().enumerate() {
            let y = area.y + offset as u16;
            let (number, _, statements) = split_number(line);
            let shown: String = statements.chars().skip(left).take(text_width).collect();
            let row = Line::from(vec![
                Span::styled(format!("{:>width$}", number, width = number_width), dim),
                Span::styled(BAR, dim),
                Span::styled(shown, plain),
            ]);
            frame.render_widget(Paragraph::new(row), Rect::new(area.x, y, area.width, 1));
            let index = top + offset;
            if let Some(mark) = self.marks.iter().find(|m| m.index == index) {
                let text = format!("◀ {}", mark.message);
                let width = (text.chars().count() as u16).min(area.width);
                let mark_area = Rect::new(area.right() - width, y, width, 1);
                frame.render_widget(Paragraph::new(text).style(red), mark_area);
            }
        }
        let cursor_x = (cursor_x as u16).min(area.width.saturating_sub(1));
        frame.set_cursor_position((area.x + cursor_x, area.y + (self.row - top) as u16));
    }
}

/// Split a line into the digits of its line number, the count of spaces
/// after them that the display leaves out (0 or 1), and the statements.
fn split_number(line: &str) -> (&str, usize, &str) {
    let digits = line.len() - line.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let (number, rest) = line.split_at(digits);
    match rest.strip_prefix(' ') {
        Some(statements) if digits > 0 => (number, 1, statements),
        _ => (number, 0, rest),
    }
}

/// The text for a typed character. A tab is 2 spaces.
fn typed_text(c: char) -> String {
    if c == '\t' {
        TAB_TEXT.to_string()
    } else {
        c.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_number_takes_the_digits_and_one_space() {
        assert_eq!(split_number("10 PRINT"), ("10", 1, "PRINT"));
        assert_eq!(split_number("10  PRINT"), ("10", 1, " PRINT"));
        assert_eq!(split_number("10PRINT"), ("10", 0, "PRINT"));
        assert_eq!(split_number(" PRINT"), ("", 0, " PRINT"));
        assert_eq!(split_number(""), ("", 0, ""));
    }

    #[test]
    fn typed_text_gives_2_spaces_for_a_tab() {
        assert_eq!(typed_text('\t'), "  ");
        assert_eq!(typed_text('A'), "A");
    }
}
