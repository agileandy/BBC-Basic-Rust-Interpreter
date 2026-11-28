//! Issue #203: the screens of the IDE show no line characters. Spaces
//! separate the line numbers from the text in EDIT and the parts of the
//! status text, the text screen in RUN has no side border, and the help
//! screen has no box.

use bbc_basic_interpreter::ui::{App, Mode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::Color;
use ratatui::Terminal;
use std::time::Duration;

/// The time limit for a run that must end.
const LIMIT: Duration = Duration::from_secs(5);
/// The sizes of the terminal for the scan of the screens.
const SIZES: [(u16, u16); 2] = [(80, 24), (60, 16)];

fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(letter: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(letter), KeyModifiers::CONTROL)
}

fn app_with(text: &str) -> App {
    let mut app = App::new(None);
    app.paste(text);
    app
}

fn draw(app: &App, width: u16, height: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
    terminal.draw(|frame| app.draw(frame)).expect("draw");
    terminal.backend().buffer().clone()
}

fn row_text(buffer: &Buffer, row: u16) -> String {
    (0..buffer.area.width)
        .map(|x| buffer[(x, row)].symbol())
        .collect()
}

fn screen_text(buffer: &Buffer) -> String {
    (0..buffer.area.height)
        .map(|row| row_text(buffer, row))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Fail if a cell of the screen holds a box-drawing character
/// (U+2500 to U+257F).
fn assert_no_line_characters(buffer: &Buffer, what: &str) {
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            let symbol = buffer[(x, y)].symbol();
            assert!(
                !symbol
                    .chars()
                    .any(|c| ('\u{2500}'..='\u{257F}').contains(&c)),
                "{what}: {symbol:?} at ({x}, {y}):\n{}",
                screen_text(buffer)
            );
        }
    }
}

/// Scan the screen of the mode and the help screen over it, at each size.
fn assert_mode_and_help_have_no_lines(app: &mut App, what: &str) {
    for (width, height) in SIZES {
        let what = format!("{what} at {width} x {height}");
        assert_no_line_characters(&draw(app, width, height), &what);
        app.key(press(KeyCode::F(1)));
        assert_no_line_characters(&draw(app, width, height), &format!("help over {what}"));
        app.key(press(KeyCode::Char('x')));
    }
}

#[test]
fn no_screen_shows_a_line_character_in_modes_0_1_and_7() {
    for mode in [0, 1, 7] {
        let mut app = app_with(&format!("10 MODE {mode}\n20 PRINT \"A\""));
        assert_eq!(app.mode(), Mode::Edit);
        assert_mode_and_help_have_no_lines(&mut app, &format!("EDIT, MODE {mode}"));
        app.key(ctrl('r'));
        assert!(app.wait_for_run(LIMIT), "the run did not end");
        assert_eq!(app.mode(), Mode::Run);
        assert_mode_and_help_have_no_lines(&mut app, &format!("RUN, MODE {mode}"));
        app.key(press(KeyCode::Tab));
        assert_eq!(app.mode(), Mode::Files);
        assert_mode_and_help_have_no_lines(&mut app, &format!("FILES, MODE {mode}"));
    }
}

#[test]
fn the_text_starts_2_columns_after_the_line_number_for_1_and_4_digits() {
    let app = app_with("1 PRINT 1\n1000 PRINT 2");
    let buffer = draw(&app, 80, 24);
    assert_eq!(row_text(&buffer, 1).trim_end(), "    1  PRINT 1");
    assert_eq!(row_text(&buffer, 2).trim_end(), " 1000  PRINT 2");
    // The line number stays dim, and the text starts at column 7
    assert_eq!(buffer[(4, 1)].fg, Color::DarkGray);
    assert_eq!(buffer[(7, 1)].symbol(), "P");
    assert_eq!(buffer[(7, 2)].symbol(), "P");
    assert_eq!(buffer[(7, 1)].fg, Color::White);
}

#[test]
fn the_status_line_of_edit_shows_the_3_parts_with_spaces() {
    // After the paste, the cursor is at the end of "20 END"
    let app = app_with("10 PRINT 1\n20 END");
    let status = row_text(&draw(&app, 100, 24), 23);
    assert!(
        status.contains("Ln 2 Col 7   2 lines   13 bytes"),
        "{status:?}"
    );
}

#[test]
fn the_help_title_is_the_first_line_in_the_highlight_colour() {
    let mut app = App::new(None);
    app.key(press(KeyCode::F(1)));
    let buffer = draw(&app, 80, 24);
    let row = row_text(&buffer, 1);
    assert!(row.starts_with("  Keys: press a key to close "), "{row:?}");
    assert_eq!(buffer[(2, 1)].symbol(), "K");
    assert_eq!(buffer[(2, 1)].fg, Color::Yellow);
}
