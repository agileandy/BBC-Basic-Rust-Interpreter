//! The EDIT pane. A placeholder: issue #170 gives it the editor.

use super::Outcome;
use crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// The state of the EDIT pane.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct EditorPane {}

impl EditorPane {
    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let style = Style::new().fg(Color::White).bg(Color::Black);
        frame.render_widget(Paragraph::new(" EDIT mode").style(style), area);
    }

    pub fn key(&mut self, _key: KeyEvent) -> Outcome {
        Outcome::Ignored
    }
}
