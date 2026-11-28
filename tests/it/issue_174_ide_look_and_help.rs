//! Issue #174: the look of the IDE. The run pane starts with the banner of a
//! BBC Micro, the text colours of COLOUR and VDU 17 show with the palette of
//! the mode, MODE sets the columns of the text screen, and F1 or Ctrl-H shows
//! a help screen. Also the 3 gaps of the files mode and the frame (D5) and
//! the dim mark of the line in work (D6).

use bbc_basic_interpreter::ui::{App, Mode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::Color;
use ratatui::Terminal;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// The time limit for a run that must end.
const LIMIT: Duration = Duration::from_secs(5);
/// The start text of the run pane.
const BANNER: [&str; 5] = ["BBC Computer 32K", "", "BASIC", "", ">"];

fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(letter: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(letter), KeyModifiers::CONTROL)
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        app.key(press(KeyCode::Char(c)));
    }
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

fn status_row(buffer: &Buffer) -> String {
    row_text(buffer, buffer.area.height - 1)
}

/// The rows of the pane at 80 x 24, with no spaces at the ends, and with
/// no empty rows at the end.
fn pane_rows(app: &App) -> Vec<String> {
    let buffer = draw(app, 80, 24);
    let mut rows: Vec<String> = (1..buffer.area.height - 1)
        .map(|row| row_text(&buffer, row).trim_matches(' ').to_string())
        .collect();
    while rows.last().is_some_and(String::is_empty) {
        rows.pop();
    }
    rows
}

/// The position of the first cell in the pane with the character.
fn find_char(buffer: &Buffer, c: char) -> (u16, u16) {
    for y in 1..buffer.area.height - 1 {
        for x in 0..buffer.area.width {
            if buffer[(x, y)].symbol() == c.to_string() {
                return (x, y);
            }
        }
    }
    panic!("no {c:?} in the pane:\n{}", screen_text(buffer));
}

/// Wait until a pane row is the text, with a time limit.
fn wait_for_row(app: &mut App, text: &str) {
    let deadline = Instant::now() + LIMIT;
    while !pane_rows(app).iter().any(|row| row == text) {
        assert!(
            Instant::now() < deadline,
            "no row {text:?}: {:?}",
            pane_rows(app)
        );
        app.wait_for_run(Duration::from_millis(10));
    }
}

/// Press Ctrl-R and wait for the end of the run.
fn run(app: &mut App) {
    app.key(ctrl('r'));
    assert!(app.wait_for_run(LIMIT), "the run did not end");
}

// The banner (D2)

#[test]
fn the_run_pane_starts_with_the_banner_of_a_bbc_micro() {
    let mut app = App::new(None);
    app.key(press(KeyCode::Tab));
    assert_eq!(app.mode(), Mode::Run);
    wait_for_row(&mut app, ">");
    assert_eq!(pane_rows(&app), BANNER);
}

#[test]
fn mode_clears_the_banner_and_the_text_before_it() {
    let mut app = app_with("10 PRINT \"A\"\n20 MODE 1\n30 PRINT \"B\"");
    run(&mut app);
    assert_eq!(pane_rows(&app), ["B", ">"]);
}

// The text colours (D1)

#[test]
fn colour_1_in_mode_1_draws_the_text_in_red() {
    let mut app = app_with("10 MODE 1\n20 COLOUR 1 : PRINT \"A\"\n30 COLOUR 2 : PRINT \"B\"");
    run(&mut app);
    let buffer = draw(&app, 80, 24);
    let a = find_char(&buffer, 'A');
    assert_eq!(buffer[a].fg, Color::Red);
    assert_eq!(buffer[a].bg, Color::Black);
    let b = find_char(&buffer, 'B');
    assert_eq!(buffer[b].fg, Color::Yellow);
}

#[test]
fn colour_1_typed_at_the_prompt_uses_the_colour_of_the_mode() {
    // Mode 0 is the start mode: 2 colours, and colour 1 is white
    let mut app = App::new(None);
    app.key(press(KeyCode::Tab));
    wait_for_row(&mut app, ">");
    type_text(&mut app, "COLOUR 1 : PRINT \"A\"");
    app.key(press(KeyCode::Enter));
    assert!(app.wait_for_run(LIMIT));
    let buffer = draw(&app, 80, 24);
    assert_eq!(buffer[find_char(&buffer, 'A')].fg, Color::White);
}

#[test]
fn vdu_17_sets_the_background_and_cls_fills_the_screen_with_it() {
    let mut app = app_with("10 MODE 2\n20 VDU 17, 129 : CLS\n30 VDU 17, 9 : PRINT \"A\"");
    run(&mut app);
    let buffer = draw(&app, 80, 24);
    let a = find_char(&buffer, 'A');
    // 16 colours: colour 9 is the steady form of colour 1
    assert_eq!(buffer[a].fg, Color::Red);
    assert_eq!(buffer[a].bg, Color::Red);
    // A cell with no text has the background of the CLS
    assert_eq!(buffer[(a.0 + 5, a.1)].bg, Color::Red);
}

#[test]
fn the_text_colours_change_back_at_mode() {
    let mut app = app_with("10 MODE 1 : COLOUR 1 : COLOUR 130\n20 MODE 1 : PRINT \"A\"");
    run(&mut app);
    let buffer = draw(&app, 80, 24);
    let a = find_char(&buffer, 'A');
    assert_eq!(buffer[a].fg, Color::White);
    assert_eq!(buffer[a].bg, Color::Black);
}

#[test]
fn a_variable_and_an_array_named_colour_still_work() {
    let mut app =
        app_with("10 colour = 3 : DIM colour(2) : colour(1) = 4\n20 PRINT colour + colour(1)");
    run(&mut app);
    assert!(
        pane_rows(&app).contains(&"7".to_string()),
        "{:?}",
        pane_rows(&app)
    );
}

// The columns of the mode (D3)

#[test]
fn mode_7_gives_a_40_column_screen_in_the_centre() {
    let mut app = app_with("10 MODE 7\n20 PRINT \"A\"; STRING$(44, \"x\"); \"Z\"");
    run(&mut app);
    let buffer = draw(&app, 80, 24);
    let (x, y) = find_char(&buffer, 'A');
    assert_eq!(x, 20, "{}", screen_text(&buffer));
    // The text wraps at 40 columns: Z is at column 6 of the next row
    assert_eq!(find_char(&buffer, 'Z'), (20 + 5, y + 1));
    // No border at the sides of the screen (issue #203)
    assert_eq!(buffer[(19, y)].symbol(), " ");
    assert_eq!(buffer[(60, y)].symbol(), " ");
}

#[test]
fn a_narrow_pane_shows_the_left_part_and_says_so_in_the_status_line() {
    let mut app = app_with("10 MODE 0\n20 PRINT STRING$(70, \"x\"); \"Z\"");
    run(&mut app);
    let buffer = draw(&app, 60, 20);
    // The 80-column screen does not wrap at the 60 columns of the pane
    let rows: Vec<String> = (1..19).map(|y| row_text(&buffer, y)).collect();
    assert!(rows.iter().any(|row| row == &"x".repeat(60)), "{rows:?}");
    assert!(!screen_text(&buffer).contains('Z'));
    let status = status_row(&buffer);
    assert!(status.contains("60 of 80 columns"), "{status:?}");
    // The wider pane shows all the screen, and the status line says nothing
    let buffer = draw(&app, 100, 24);
    assert!(screen_text(&buffer).contains(&format!("{}Z", "x".repeat(70))));
    assert!(!status_row(&buffer).contains("columns"));
}

// The help screen (D4)

#[test]
fn f1_shows_the_help_screen_and_the_next_key_closes_it() {
    let mut app = App::new(None);
    app.key(press(KeyCode::F(1)));
    let text = screen_text(&draw(&app, 80, 24));
    for word in [
        "EDIT", "RUN", "FILES", "Ctrl-R", "Ctrl-N", "Escape", "Delete",
    ] {
        assert!(text.contains(word), "{word} is not in:\n{text}");
    }
    // The key that closes the help does nothing else
    app.key(press(KeyCode::Char('x')));
    assert!(!screen_text(&draw(&app, 80, 24)).contains("Ctrl-N"));
    assert_eq!(app.document().lines(), [""]);
    assert_eq!(app.mode(), Mode::Edit);
}

#[test]
fn ctrl_h_shows_the_help_screen_over_each_mode() {
    let mut app = App::new(None);
    for mode in [Mode::Edit, Mode::Run, Mode::Files] {
        assert_eq!(app.mode(), mode);
        app.key(ctrl('h'));
        assert!(screen_text(&draw(&app, 80, 24)).contains("Ctrl-N"));
        // Tab closes the help and does not change the mode
        app.key(press(KeyCode::Tab));
        assert_eq!(app.mode(), mode);
        assert!(!screen_text(&draw(&app, 80, 24)).contains("Ctrl-N"));
        app.key(press(KeyCode::Tab));
    }
}

#[test]
fn the_status_line_shows_f1_help() {
    let app = App::new(None);
    assert!(status_row(&draw(&app, 80, 24)).contains("F1 help  Tab mode  ^Q quit"));
}

// The gaps of the files mode and the frame (D5)

#[test]
fn the_status_message_clears_at_the_next_key() {
    let mut app = app_with("10 PRINT \\");
    app.key(ctrl('n'));
    assert_eq!(app.message(), "Correct the marked lines first");
    app.key(press(KeyCode::Left));
    assert_eq!(app.message(), "");
}

#[test]
fn the_quit_question_stays_a_question() {
    let mut app = app_with("10 END");
    app.set_changed(true);
    app.key(ctrl('q'));
    assert!(app.message().starts_with("Not saved"), "{}", app.message());
    assert!(!app.should_quit());
    app.key(ctrl('q'));
    assert!(app.should_quit());
}

#[test]
fn a_long_file_name_shows_its_last_part_and_the_line_and_column_stay() {
    let name = format!("/{}/end.bbas", "long".repeat(30));
    let app = App::new(Some(name));
    let status = status_row(&draw(&app, 80, 24));
    assert!(status.contains("…"), "{status:?}");
    assert!(status.contains("end.bbas"), "{status:?}");
    assert!(status.contains("Ln 1 Col 1"), "{status:?}");
}

#[cfg(unix)]
#[test]
fn a_save_keeps_the_permissions_of_the_old_file() {
    use std::os::unix::fs::PermissionsExt;
    let dir: PathBuf = std::env::temp_dir().join(format!("issue_174_{}_mode", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir(&dir).expect("test directory is made");
    let path = dir.join("keep.bbas");
    std::fs::write(&path, "10 END\n").expect("file is written");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o751)).expect("mode");
    let mut app = App::new(Some(path.display().to_string()));
    app.paste("\n20 PRINT 1");
    app.key(ctrl('s'));
    assert_eq!(app.message(), "Saved keep.bbas");
    let mode = std::fs::metadata(&path).expect("file").permissions().mode();
    assert_eq!(mode & 0o777, 0o751);
    let _ = std::fs::remove_dir_all(&dir);
}

// The marks of the edit mode (D6)

#[test]
fn the_mark_of_the_line_in_work_is_dim_and_other_marks_are_red() {
    let app = app_with("10 PRINT \\\n20 PRINT \\");
    assert_eq!(app.cursor().0, 1);
    let buffer = draw(&app, 80, 24);
    let mark_x = |row: u16| {
        (0..80)
            .find(|&x| buffer[(x, row)].symbol() == "◀")
            .unwrap_or_else(|| panic!("no mark in row {row}"))
    };
    assert_eq!(buffer[(mark_x(1), 1)].fg, Color::Red);
    assert_eq!(buffer[(mark_x(2), 2)].fg, Color::DarkGray);
}
