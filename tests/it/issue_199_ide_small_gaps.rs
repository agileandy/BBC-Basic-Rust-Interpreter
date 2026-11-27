//! Issue #199: 5 small gaps of the IDE. The help screen scrolls in a small
//! window, QUIT and EXIT show a message and the IDE stays open, a run shows
//! in the status line of each mode, a refused typed line leaves one prompt,
//! and COLOUR does nothing in mode 7.

use bbc_basic_interpreter::ui::{App, Mode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::Color;
use ratatui::Terminal;
use std::time::{Duration, Instant};

/// The time limit for a run that must end.
const LIMIT: Duration = Duration::from_secs(5);
/// The start text of the run pane, before the first prompt.
const BANNER: [&str; 4] = ["BBC Computer 32K", "", "BASIC", ""];
/// The status message after QUIT or EXIT.
const QUIT_MESSAGE: &str = "The program asked to quit: press Ctrl-Q to leave the IDE";

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

fn status_row(app: &App) -> String {
    let buffer = draw(app, 80, 24);
    row_text(&buffer, buffer.area.height - 1)
}

/// The rows of the pane at 80 x 24, with no spaces at the ends, with no
/// empty rows at the end, and with no banner at the top.
fn pane_rows(app: &App) -> Vec<String> {
    let buffer = draw(app, 80, 24);
    let mut rows: Vec<String> = (1..buffer.area.height - 1)
        .map(|row| row_text(&buffer, row).trim_matches([' ', '│']).to_string())
        .collect();
    if rows.starts_with(&BANNER.map(String::from)) {
        rows.drain(..BANNER.len());
    }
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
    panic!("no {c:?} in:\n{}", screen_text(buffer));
}

/// Press Ctrl-R and wait for the end of the run.
fn run(app: &mut App) {
    app.key(ctrl('r'));
    assert!(app.wait_for_run(LIMIT), "the run did not end");
}

/// Wait until the last pane row is the prompt, with a time limit.
fn wait_for_prompt(app: &mut App) {
    let deadline = Instant::now() + LIMIT;
    while pane_rows(app).last().map(String::as_str) != Some(">") {
        assert!(Instant::now() < deadline, "no prompt: {:?}", pane_rows(app));
        app.wait_for_run(Duration::from_millis(10));
    }
}

// (1) The help screen at the smallest window

/// Each key of the help screen with what it does.
const HELP_KEYS: [(&str, &str); 27] = [
    ("Tab", "next mode"),
    ("Shift-Tab", "mode before"),
    ("Ctrl-R", "run the program"),
    ("Ctrl-S", "save"),
    ("Ctrl-O", "files"),
    ("Ctrl-Q", "quit"),
    ("F1 Ctrl-H", "this help"),
    ("Arrows", "move"),
    ("PgUp PgDn", "page"),
    ("Home End", "line start, end"),
    ("Ctrl-Home", "text start"),
    ("Ctrl-End", "text end"),
    ("Enter", "new line"),
    ("Backspace", "delete left"),
    ("Delete", "delete right"),
    ("Ctrl-N", "renumber"),
    ("Keys", "type a command"),
    ("Enter", "run the command"),
    ("Escape", "stop a program"),
    ("PgUp PgDn", "scroll"),
    ("Up Down", "select"),
    ("PgUp PgDn", "page"),
    ("Enter", "open"),
    ("s", "save as"),
    ("n", "new program"),
    ("d", "delete"),
    ("y", "answer yes"),
];

/// The help lines that are not in the text.
fn missing_help_keys(text: &str) -> Vec<String> {
    HELP_KEYS
        .iter()
        .map(|(key, action)| format!("{:<10}{}", key, action))
        .filter(|line| !text.contains(line.as_str()))
        .collect()
}

#[test]
fn the_help_at_80_by_24_shows_each_key_with_no_scroll() {
    let mut app = App::new(None);
    app.key(press(KeyCode::F(1)));
    let text = screen_text(&draw(&app, 80, 24));
    assert_eq!(missing_help_keys(&text), Vec::<String>::new(), "\n{text}");
}

#[test]
fn the_help_at_60_by_16_shows_each_key_after_a_scroll() {
    let mut app = App::new(None);
    app.key(press(KeyCode::F(1)));
    let mut seen = screen_text(&draw(&app, 60, 16));
    for _ in 0..HELP_KEYS.len() * 2 {
        app.key(press(KeyCode::Down));
        seen.push_str(&screen_text(&draw(&app, 60, 16)));
    }
    assert_eq!(missing_help_keys(&seen), Vec::<String>::new(), "\n{seen}");
    // Up goes back to the top, and the help stays open
    for _ in 0..HELP_KEYS.len() * 2 {
        app.key(press(KeyCode::Up));
    }
    let text = screen_text(&draw(&app, 60, 16));
    assert!(text.contains("ALL MODES"), "\n{text}");
    // Another key closes the help, and does nothing else
    app.key(press(KeyCode::Char('x')));
    assert!(!screen_text(&draw(&app, 60, 16)).contains("ALL MODES"));
    assert_eq!(app.document().lines(), [""]);
}

#[test]
fn the_page_keys_scroll_the_help_by_a_page() {
    let mut app = App::new(None);
    app.key(press(KeyCode::F(1)));
    let mut seen = screen_text(&draw(&app, 60, 16));
    for _ in 0..5 {
        app.key(press(KeyCode::PageDown));
        seen.push_str(&screen_text(&draw(&app, 60, 16)));
    }
    assert_eq!(missing_help_keys(&seen), Vec::<String>::new(), "\n{seen}");
    app.key(press(KeyCode::PageUp));
    app.key(press(KeyCode::PageUp));
    app.key(press(KeyCode::PageUp));
    app.key(press(KeyCode::PageUp));
    app.key(press(KeyCode::PageUp));
    assert!(screen_text(&draw(&app, 60, 16)).contains("ALL MODES"));
}

// (2) QUIT and EXIT in the IDE

#[test]
fn quit_in_a_program_shows_the_message_and_the_ide_stays_open() {
    let mut app = app_with("10 PRINT \"A\"\n20 QUIT\n30 PRINT \"B\"");
    run(&mut app);
    assert!(!app.should_quit());
    assert_eq!(app.mode(), Mode::Run);
    assert_eq!(app.message(), QUIT_MESSAGE);
    assert!(
        status_row(&app).contains(QUIT_MESSAGE),
        "{:?}",
        status_row(&app)
    );
    assert_eq!(pane_rows(&app), ["> RUN", "A", ">"]);
    // The pane stays live: a typed line runs
    type_text(&mut app, "PRINT ;7");
    app.key(press(KeyCode::Enter));
    assert!(app.wait_for_run(LIMIT));
    assert_eq!(app.message(), "");
    assert_eq!(pane_rows(&app), ["> RUN", "A", "> PRINT ;7", "7", ">"]);
}

#[test]
fn exit_typed_in_the_run_pane_shows_the_message_and_the_ide_stays_open() {
    for typed in ["EXIT", "QUIT"] {
        let mut app = App::new(None);
        app.key(press(KeyCode::Tab));
        wait_for_prompt(&mut app);
        type_text(&mut app, typed);
        app.key(press(KeyCode::Enter));
        assert!(app.wait_for_run(LIMIT), "{typed}");
        assert!(!app.should_quit(), "{typed}");
        assert_eq!(app.message(), QUIT_MESSAGE, "{typed}");
    }
}

#[test]
fn ctrl_q_after_quit_leaves_the_ide() {
    let mut app = app_with("10 QUIT");
    app.set_changed(false);
    run(&mut app);
    app.key(ctrl('q'));
    assert!(app.should_quit());
}

// (3) A run shows in each mode

#[test]
fn a_run_in_progress_shows_in_the_edit_and_the_files_tabs() {
    let mut app = app_with("10 A$ = GET$");
    app.key(ctrl('r'));
    assert!(app.is_running());
    app.key(press(KeyCode::BackTab));
    assert_eq!(app.mode(), Mode::Edit);
    assert!(
        status_row(&app).contains("running"),
        "{:?}",
        status_row(&app)
    );
    // The text of the editor stays in the status line
    assert!(
        status_row(&app).contains("Ln 1 Col"),
        "{:?}",
        status_row(&app)
    );
    app.key(ctrl('o'));
    assert_eq!(app.mode(), Mode::Files);
    assert!(
        status_row(&app).contains("running"),
        "{:?}",
        status_row(&app)
    );
    // Escape in the run mode stops the program: no run shows after it
    app.key(press(KeyCode::Tab));
    app.key(press(KeyCode::Tab));
    assert_eq!(app.mode(), Mode::Run);
    app.key(press(KeyCode::Esc));
    assert!(app.wait_for_run(LIMIT));
    app.key(press(KeyCode::Tab));
    assert_eq!(app.mode(), Mode::Files);
    assert!(
        !status_row(&app).contains("running"),
        "{:?}",
        status_row(&app)
    );
    app.key(press(KeyCode::Tab));
    assert_eq!(app.mode(), Mode::Edit);
    assert!(
        !status_row(&app).contains("running"),
        "{:?}",
        status_row(&app)
    );
}

// (4) One prompt after a refused line

#[test]
fn a_refused_typed_line_leaves_one_prompt() {
    let mut app = app_with("10 PRINT 1\n20 PRNT 2 +");
    app.key(press(KeyCode::Tab));
    wait_for_prompt(&mut app);
    type_text(&mut app, "RUN");
    app.key(press(KeyCode::Enter));
    assert!(app.wait_for_run(LIMIT));
    assert_eq!(app.mode(), Mode::Edit);
    app.key(press(KeyCode::Tab));
    wait_for_prompt(&mut app);
    assert_eq!(pane_rows(&app), [">"]);
    // A second refused line also leaves one prompt
    type_text(&mut app, "PRINT ;2");
    app.key(press(KeyCode::Enter));
    assert!(app.wait_for_run(LIMIT));
    app.key(press(KeyCode::Tab));
    wait_for_prompt(&mut app);
    assert_eq!(pane_rows(&app), [">"]);
}

#[test]
fn a_line_typed_after_a_refused_line_runs_on_the_same_prompt() {
    let mut app = app_with("10 PRNT 1");
    app.key(press(KeyCode::Tab));
    wait_for_prompt(&mut app);
    type_text(&mut app, "LIST");
    app.key(press(KeyCode::Enter));
    assert!(app.wait_for_run(LIMIT));
    // Correct the fault, then go back to the run mode
    app.key(press(KeyCode::Home));
    app.key(press(KeyCode::Right));
    app.key(press(KeyCode::Right));
    app.key(press(KeyCode::Right));
    app.key(press(KeyCode::Right));
    app.key(press(KeyCode::Right));
    type_text(&mut app, "I");
    assert_eq!(app.document().lines(), ["10 PRINT 1"]);
    app.key(press(KeyCode::Tab));
    type_text(&mut app, "LIST");
    app.key(press(KeyCode::Enter));
    assert!(app.wait_for_run(LIMIT));
    assert_eq!(pane_rows(&app), ["> LIST", "10 PRINT 1", ">"]);
}

#[test]
fn enter_after_an_error_goes_to_edit_and_leaves_one_prompt() {
    let mut app = app_with("10 X = 1 / 0");
    run(&mut app);
    app.key(press(KeyCode::Enter));
    assert!(app.wait_for_run(LIMIT));
    assert_eq!(app.mode(), Mode::Edit);
    app.key(press(KeyCode::Tab));
    wait_for_prompt(&mut app);
    assert_eq!(
        pane_rows(&app),
        ["> RUN", "Error: Division by zero at line 10", ">"]
    );
}

// (5) COLOUR in mode 7

#[test]
fn colour_in_mode_7_does_nothing() {
    let mut app = app_with("10 MODE 7 : COLOUR 1 : PRINT \"A\"\n20 COLOUR 130 : PRINT \"B\"");
    run(&mut app);
    let buffer = draw(&app, 80, 24);
    for c in ['A', 'B'] {
        let cell = &buffer[find_char(&buffer, c)];
        assert_eq!(cell.fg, Color::White, "{c}");
        assert_eq!(cell.bg, Color::Black, "{c}");
    }
}

#[test]
fn vdu_17_in_mode_7_does_nothing() {
    let mut app = app_with("10 MODE 7 : VDU 17, 2 : PRINT \"A\"");
    run(&mut app);
    let buffer = draw(&app, 80, 24);
    assert_eq!(buffer[find_char(&buffer, 'A')].fg, Color::White);
}

#[test]
fn colour_in_the_text_only_mode_6_still_works() {
    // Mode 6 has 2 colours: colour 0 is black
    let mut app = app_with("10 MODE 6 : COLOUR 129 : COLOUR 0 : PRINT \"A\"");
    run(&mut app);
    let buffer = draw(&app, 80, 24);
    let cell = &buffer[find_char(&buffer, 'A')];
    assert_eq!(cell.fg, Color::Black);
    assert_eq!(cell.bg, Color::White);
}
