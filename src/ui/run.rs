//! The RUN pane. A placeholder: issue #171 gives it the run of a program.

use super::Outcome;
use crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// The state of the RUN pane.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RunPane {}

impl RunPane {
    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let style = Style::new().fg(Color::White).bg(Color::Black);
        frame.render_widget(Paragraph::new(" RUN mode").style(style), area);
    }

    pub fn key(&mut self, _key: KeyEvent) -> Outcome {
        Outcome::Ignored
    }
}
