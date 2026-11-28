//! The help screen (issue #174): F1 or Ctrl-H shows it over the current
//! mode, and each key closes it. One table has the keys of the frame and of
//! the 3 modes.
//!
//! If the 3 columns do not fit the pane, the help shows one column. If that
//! column is taller than the pane, the arrow keys and the page keys scroll
//! it, and the other keys close it (issue #199).

use super::theme;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Padding, Paragraph};
use ratatui::Frame;
use std::cell::Cell;

/// One group of keys: the title, then each key with what it does.
struct Group {
    title: &'static str,
    keys: &'static [(&'static str, &'static str)],
}

/// The keys of the frame, in each mode.
const GLOBAL: Group = Group {
    title: "ALL MODES",
    keys: &[
        ("Tab", "next mode"),
        ("Shift-Tab", "mode before"),
        ("Ctrl-R", "run the program"),
        ("Ctrl-S", "save"),
        ("Ctrl-O", "files"),
        ("Ctrl-Q", "quit"),
        ("F1 Ctrl-H", "this help"),
    ],
};

/// The keys of each mode, in the order of the tab bar.
const MODES: [Group; 3] = [
    Group {
        title: "EDIT",
        keys: &[
            ("Arrows", "move"),
            ("PgUp PgDn", "page"),
            ("Home End", "line start, end"),
            ("Ctrl-Home", "text start"),
            ("Ctrl-End", "text end"),
            ("Enter", "new line"),
            ("Backspace", "delete left"),
            ("Delete", "delete right"),
            ("Ctrl-N", "renumber"),
        ],
    },
    Group {
        title: "RUN",
        keys: &[
            ("Keys", "type a command"),
            ("Enter", "run the command"),
            ("Escape", "stop a program"),
            ("PgUp PgDn", "scroll"),
        ],
    },
    Group {
        title: "FILES",
        keys: &[
            ("Up Down", "select"),
            ("PgUp PgDn", "page"),
            ("Enter", "open"),
            ("s", "save as"),
            ("n", "new program"),
            ("d", "delete"),
            ("y", "answer yes"),
        ],
    },
];

/// The width of the key column.
const KEY_WIDTH: usize = 10;

/// The lines of one group.
fn group_lines(group: &Group) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(Span::styled(group.title, theme::highlight()))];
    lines.extend(group.keys.iter().map(|(key, action)| {
        Line::from(vec![
            Span::styled(format!("{:<KEY_WIDTH$}", key), theme::info()),
            Span::styled(*action, theme::text()),
        ])
    }));
    lines
}

/// The width of one column: the key column and the longest action.
fn column_width() -> u16 {
    let longest = GLOBAL
        .keys
        .iter()
        .chain(MODES.iter().flat_map(|group| group.keys))
        .map(|(_, action)| action.len())
        .max()
        .unwrap_or(0);
    (KEY_WIDTH + longest) as u16
}

/// The count of rows of the keys of the frame in 3 columns, with the title
/// and one empty row.
fn global_rows() -> u16 {
    GLOBAL.keys.len().div_ceil(3) as u16 + 2
}

/// True if the 3 columns fit in the area.
fn columns_fit(area: Rect) -> bool {
    let mode_rows = MODES
        .iter()
        .map(|group| group.keys.len())
        .max()
        .unwrap_or(0) as u16
        + 1;
    area.width >= 3 * column_width() + 2 && area.height >= global_rows() + mode_rows
}

/// The state of the help screen: the scroll of the one column.
#[derive(Default)]
pub(super) struct Help {
    /// The count of lines above the top of the pane
    scroll: usize,
    /// The largest scroll and the count of lines in the pane at the last
    /// draw. 0 if all the help shows.
    max_scroll: Cell<usize>,
    page: Cell<usize>,
}

impl Help {
    /// Scroll for an arrow key or a page key, if the help scrolls. Give
    /// false for the other keys: they close the help.
    pub(super) fn key(&mut self, key: KeyEvent) -> bool {
        let max = self.max_scroll.get();
        if max == 0 {
            return false;
        }
        let page = self.page.get().max(1);
        self.scroll = match key.code {
            KeyCode::Up => self.scroll.saturating_sub(1),
            KeyCode::Down => self.scroll + 1,
            KeyCode::PageUp => self.scroll.saturating_sub(page),
            KeyCode::PageDown => self.scroll + page,
            _ => return false,
        }
        .min(max);
        true
    }

    /// Draw the help screen over the area of the pane.
    pub(super) fn draw(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Clear, area);
        let inner = help_block().inner(area);
        if columns_fit(inner) {
            self.max_scroll.set(0);
            draw_block(frame, area, " Keys: press a key to close ");
            draw_columns(frame, inner);
            return;
        }
        // One column, with an empty line between the groups
        let mut lines = vec![];
        for group in std::iter::once(&GLOBAL).chain(MODES.iter()) {
            if !lines.is_empty() {
                lines.push(Line::default());
            }
            lines.extend(group_lines(group));
        }
        let page = usize::from(inner.height);
        let max = lines.len().saturating_sub(page);
        self.max_scroll.set(max);
        self.page.set(page);
        let title = if max == 0 {
            " Keys: press a key to close "
        } else {
            " Keys: arrows scroll, other keys close "
        };
        draw_block(frame, area, title);
        let scroll = self.scroll.min(max) as u16;
        frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)), inner);
    }
}

/// The background of the help screen, with no border (issue #203). The
/// first row is for the title, and one column at each side stays empty.
fn help_block() -> Block<'static> {
    Block::new()
        .padding(Padding::new(1, 1, 1, 0))
        .style(theme::text())
}

/// The background and the title of the help screen.
fn draw_block(frame: &mut Frame, area: Rect, title: &'static str) {
    frame.render_widget(help_block(), area);
    let title_area = Rect::new(area.x + 1, area.y, area.width.saturating_sub(1), 1);
    let title = Line::from(Span::styled(title, theme::highlight()));
    frame.render_widget(Paragraph::new(title), title_area);
}

/// The keys of the frame in 3 columns, and the keys of each mode in one
/// column.
fn draw_columns(frame: &mut Frame, inner: Rect) {
    let global_rows = global_rows();
    let [global, modes] =
        Layout::vertical([Constraint::Length(global_rows), Constraint::Min(0)]).areas(inner);
    // The keys of the frame, in 3 columns under one title
    let [title, global] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(global);
    let title_line = Line::from(Span::styled(GLOBAL.title, theme::highlight()));
    frame.render_widget(Paragraph::new(title_line), title);
    let columns = Layout::horizontal([Constraint::Ratio(1, 3); 3])
        .spacing(1)
        .areas::<3>(global);
    let rows = GLOBAL.keys.len().div_ceil(3);
    for (index, column) in columns.into_iter().enumerate() {
        let group = Group {
            title: "",
            keys: &GLOBAL.keys[(index * rows).min(GLOBAL.keys.len())
                ..((index + 1) * rows).min(GLOBAL.keys.len())],
        };
        let lines: Vec<Line> = group_lines(&group).into_iter().skip(1).collect();
        frame.render_widget(Paragraph::new(lines), column);
    }
    // The keys of the 3 modes, one mode in each column
    let columns = Layout::horizontal([Constraint::Ratio(1, 3); 3])
        .spacing(1)
        .areas::<3>(modes);
    for (group, column) in MODES.iter().zip(columns) {
        frame.render_widget(Paragraph::new(group_lines(group)), column);
    }
}
