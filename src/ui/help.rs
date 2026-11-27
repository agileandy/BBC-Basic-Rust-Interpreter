//! The help screen (issue #174): F1 or Ctrl-H shows it over the current
//! mode, and each key closes it. One table has the keys of the frame and of
//! the 3 modes.

use super::theme;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

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

/// Draw the help screen over the area of the pane.
pub(super) fn draw(frame: &mut Frame, area: Rect) {
    frame.render_widget(Clear, area);
    let block = Block::new()
        .borders(Borders::ALL)
        .border_style(theme::dim())
        .title(Span::styled(
            " Keys: press a key to close ",
            theme::highlight(),
        ))
        .style(theme::text());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let global_rows = GLOBAL.keys.len().div_ceil(3) as u16 + 2;
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
