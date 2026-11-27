//! The FILES pane. A placeholder: issue #172 gives it the file list.

use super::Outcome;
use crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// The state of the FILES pane.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct FilesPane {}

impl FilesPane {
    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let style = Style::new().fg(Color::White).bg(Color::Black);
        frame.render_widget(Paragraph::new(" FILES mode").style(style), area);
    }

    pub fn key(&mut self, _key: KeyEvent) -> Outcome {
        Outcome::Ignored
    }
}
