//! The tab bar and the status line of the IDE.

use super::{Mode, MIN_HEIGHT, MIN_WIDTH};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// The keys that the status line always shows, at the right.
const KEYS: &str = "Tab mode  ^Q quit ";

/// White text on black: the colours of the frame.
fn plain() -> Style {
    Style::new().fg(Color::White).bg(Color::Black)
}

/// The values that the status line shows.
pub(super) struct Status<'a> {
    pub mode: Mode,
    pub file_name: Option<&'a str>,
    pub changed: bool,
    pub message: &'a str,
    /// The text of the pane, such as the cursor position. Empty for none.
    pub detail: &'a str,
}

/// One line that asks for a larger window.
pub(super) fn draw_too_small(frame: &mut Frame, area: Rect) {
    let text = format!("Make the window larger ({} x {})", MIN_WIDTH, MIN_HEIGHT);
    frame.render_widget(Paragraph::new(text).style(plain()), area);
}

/// The tabs EDIT, RUN and FILES, with the tab of the current mode in reverse.
pub(super) fn draw_tabs(frame: &mut Frame, area: Rect, current: Mode) {
    let spans: Vec<Span> = Mode::ALL
        .iter()
        .flat_map(|&mode| {
            let style = if mode == current {
                plain().add_modifier(Modifier::REVERSED)
            } else {
                plain()
            };
            [
                Span::styled(" ", plain()),
                Span::styled(format!(" {} ", mode.title()), style),
            ]
        })
        .collect();
    frame.render_widget(Paragraph::new(Line::from(spans)).style(plain()), area);
}

/// The mode in yellow, then the file name, the changed mark, the message, the
/// text of the pane and the keys in cyan, all on black.
pub(super) fn draw_status(frame: &mut Frame, area: Rect, status: &Status) {
    let yellow = Style::new().fg(Color::Yellow).bg(Color::Black);
    let cyan = Style::new().fg(Color::Cyan).bg(Color::Black);
    let name = status.file_name.unwrap_or("(no name)");
    let mark = if status.changed { " *" } else { "" };
    let mut left = Line::from(vec![
        Span::styled(format!(" {} ", status.mode.title()), yellow),
        Span::styled(format!(" {}{}  {}", name, mark, status.message), cyan),
    ]);
    if !status.detail.is_empty() {
        left.push_span(Span::styled(format!("  {}", status.detail), cyan));
    }
    // The message is more important than the keys: show the keys only if
    // they fit after it, with 2 spaces between.
    let keys_width = KEYS.len() as u16;
    if left.width() as u16 + 2 + keys_width > area.width {
        frame.render_widget(Paragraph::new(left).style(cyan), area);
        return;
    }
    let [left_area, keys_area] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(keys_width)]).areas(area);
    frame.render_widget(Paragraph::new(left).style(cyan), left_area);
    frame.render_widget(Paragraph::new(KEYS).style(cyan), keys_area);
}
