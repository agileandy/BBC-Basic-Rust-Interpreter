//! The tab bar and the status line of the IDE.

use super::{theme, Mode, MIN_HEIGHT, MIN_WIDTH};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// The keys that the status line always shows, at the right.
const KEYS: &str = "F1 help  Tab mode  ^Q quit ";
/// The mark at the start of a file name that shows only its last part.
const CUT_MARK: char = '…';

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
    frame.render_widget(Paragraph::new(text).style(theme::text()), area);
}

/// The tabs EDIT, RUN and FILES, with the tab of the current mode in reverse.
pub(super) fn draw_tabs(frame: &mut Frame, area: Rect, current: Mode) {
    let plain = theme::text();
    let spans: Vec<Span> = Mode::ALL
        .iter()
        .flat_map(|&mode| {
            let style = if mode == current {
                plain.add_modifier(Modifier::REVERSED)
            } else {
                plain
            };
            [
                Span::styled(" ", plain),
                Span::styled(format!(" {} ", mode.title()), style),
            ]
        })
        .collect();
    frame.render_widget(Paragraph::new(Line::from(spans)).style(plain), area);
}

/// The mode in yellow, then the file name, the changed mark, the message, the
/// text of the pane and the keys in cyan, all on black.
pub(super) fn draw_status(frame: &mut Frame, area: Rect, status: &Status) {
    let yellow = theme::highlight();
    let cyan = theme::info();
    let mark = if status.changed { " *" } else { "" };
    let mode = Span::styled(format!(" {} ", status.mode.title()), yellow);
    let rest = format!("{}  {}", mark, status.message);
    let detail = if status.detail.is_empty() {
        String::new()
    } else {
        format!("  {}", status.detail)
    };
    // A file name that is too long shows its last part, so that the text
    // of the pane, such as the line and the column, stays
    let others = mode.width() + 1 + rest.chars().count() + detail.chars().count();
    let name = match status.file_name {
        Some(name) => last_part(name, usize::from(area.width).saturating_sub(others)),
        None => "(no name)".to_string(),
    };
    let mut left = Line::from(vec![mode, Span::styled(format!(" {}{}", name, rest), cyan)]);
    if !detail.is_empty() {
        left.push_span(Span::styled(detail, cyan));
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

/// The name, or the mark and the last part of the name if the name is wider
/// than `width`.
fn last_part(name: &str, width: usize) -> String {
    let count = name.chars().count();
    if count <= width {
        return name.to_string();
    }
    let keep = width.saturating_sub(1);
    std::iter::once(CUT_MARK)
        .chain(name.chars().skip(count - keep))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_part_keeps_the_end_of_a_long_name() {
        assert_eq!(last_part("abc.bbas", 8), "abc.bbas");
        assert_eq!(last_part("/long/abc.bbas", 9), "…abc.bbas");
        assert_eq!(last_part("abc", 0), "…");
    }
}
