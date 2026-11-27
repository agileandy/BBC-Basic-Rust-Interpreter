//! Issue #169: the frame of the IDE. A tab bar for EDIT, RUN and FILES,
//! a status line, Tab and Shift-Tab to change the mode, Ctrl-Q to quit.
//! The screen tests use the test backend of ratatui, with no real terminal.

use bbc_basic_interpreter::ui::{App, Mode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};
use ratatui::Terminal;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(letter: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(letter), KeyModifiers::CONTROL)
}

fn shift_tab() -> KeyEvent {
    KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)
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

/// The column of the first cell of `word` in the row.
fn column_of(buffer: &Buffer, row: u16, word: &str) -> u16 {
    let text = row_text(buffer, row);
    text.find(word)
        .unwrap_or_else(|| panic!("{word} in {text:?}")) as u16
}

fn is_reversed(buffer: &Buffer, row: u16, word: &str) -> bool {
    let x = column_of(buffer, row, word);
    buffer[(x, row)].modifier.contains(Modifier::REVERSED)
}

#[test]
fn start_screen_shows_the_three_tabs_with_edit_current() {
    let app = App::new(None);
    assert_eq!(app.mode(), Mode::Edit);
    let buffer = draw(&app, 80, 24);
    let tabs = row_text(&buffer, 0);
    assert!(tabs.contains("EDIT"), "{tabs:?}");
    assert!(tabs.contains("RUN"), "{tabs:?}");
    assert!(tabs.contains("FILES"), "{tabs:?}");
    assert!(is_reversed(&buffer, 0, "EDIT"));
    assert!(!is_reversed(&buffer, 0, "RUN"));
    assert!(!is_reversed(&buffer, 0, "FILES"));
}

#[test]
fn tab_changes_to_run_then_files_then_edit() {
    let mut app = App::new(None);
    app.key(press(KeyCode::Tab));
    assert_eq!(app.mode(), Mode::Run);
    assert!(is_reversed(&draw(&app, 80, 24), 0, "RUN"));
    app.key(press(KeyCode::Tab));
    assert_eq!(app.mode(), Mode::Files);
    assert!(is_reversed(&draw(&app, 80, 24), 0, "FILES"));
    app.key(press(KeyCode::Tab));
    assert_eq!(app.mode(), Mode::Edit);
    assert!(is_reversed(&draw(&app, 80, 24), 0, "EDIT"));
}

#[test]
fn shift_tab_goes_back() {
    let mut app = App::new(None);
    app.key(shift_tab());
    assert_eq!(app.mode(), Mode::Files);
    app.key(shift_tab());
    assert_eq!(app.mode(), Mode::Run);
    app.key(shift_tab());
    assert_eq!(app.mode(), Mode::Edit);
}

#[test]
fn each_mode_shows_its_placeholder_pane() {
    let mut app = App::new(None);
    // Issue #170 gives the edit mode the editor in place of a placeholder.
    assert!(screen_text(&draw(&app, 80, 24)).contains("Ln 1 Col 1"));
    app.key(press(KeyCode::Tab));
    assert!(screen_text(&draw(&app, 80, 24)).contains("RUN mode"));
    // Issue #172 gives the files mode the file list.
    app.key(press(KeyCode::Tab));
    assert!(screen_text(&draw(&app, 80, 24)).contains("Enter open"));
}

#[test]
fn status_line_shows_the_mode_the_name_and_the_keys() {
    let app = App::new(None);
    let buffer = draw(&app, 80, 24);
    let status = row_text(&buffer, 23);
    assert!(status.contains("EDIT"), "{status:?}");
    assert!(status.contains("(no name)"), "{status:?}");
    assert!(status.contains("Tab mode  ^Q quit"), "{status:?}");
}

#[test]
fn status_line_shows_the_file_name_from_the_start() {
    let app = App::new(Some("hello.bbas".to_string()));
    let status = row_text(&draw(&app, 80, 24), 23);
    assert!(status.contains("hello.bbas"), "{status:?}");
    assert!(!status.contains("(no name)"), "{status:?}");
}

#[test]
fn status_line_uses_the_yellow_and_cyan_of_mode_7() {
    let app = App::new(None);
    let buffer = draw(&app, 80, 24);
    let mode_x = column_of(&buffer, 23, "EDIT");
    let keys_x = column_of(&buffer, 23, "Tab mode");
    assert_eq!(buffer[(mode_x, 23)].fg, Color::Yellow);
    assert_eq!(buffer[(keys_x, 23)].fg, Color::Cyan);
    assert_eq!(buffer[(keys_x, 23)].bg, Color::Black);
}

#[test]
fn frame_text_is_white_on_black() {
    let app = App::new(None);
    let buffer = draw(&app, 80, 24);
    let x = column_of(&buffer, 0, "RUN");
    assert_eq!(buffer[(x, 0)].fg, Color::White);
    assert_eq!(buffer[(x, 0)].bg, Color::Black);
}

#[test]
fn a_resize_draws_the_frame_again() {
    let app = App::new(None);
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("test terminal");
    terminal.draw(|frame| app.draw(frame)).expect("draw");
    terminal.backend_mut().resize(100, 30);
    terminal.draw(|frame| app.draw(frame)).expect("draw");
    let buffer = terminal.backend().buffer().clone();
    assert_eq!(buffer.area.width, 100);
    assert!(row_text(&buffer, 0).contains("FILES"));
    assert!(row_text(&buffer, 29).contains("Tab mode  ^Q quit"));
}

#[test]
fn a_small_terminal_shows_one_line_that_asks_for_a_larger_window() {
    let app = App::new(None);
    for (width, height) in [(59, 24), (80, 15)] {
        let text = screen_text(&draw(&app, width, height));
        assert!(text.contains("Make the window larger"), "{text:?}");
        assert!(!text.contains("FILES"), "{text:?}");
    }
    let text = screen_text(&draw(&app, 60, 16));
    assert!(text.contains("FILES"), "{text:?}");
}

#[test]
fn ctrl_q_with_no_changes_ends() {
    let mut app = App::new(None);
    assert!(!app.should_quit());
    app.key(ctrl('q'));
    assert!(app.should_quit());
}

#[test]
fn ctrl_q_with_changes_asks_first() {
    let mut app = App::new(None);
    app.set_changed(true);
    app.key(ctrl('q'));
    assert!(!app.should_quit());
    assert_eq!(app.message(), "Not saved. Press ^Q again to quit");
    app.key(ctrl('q'));
    assert!(app.should_quit());
}

#[test]
fn another_key_after_the_question_cancels_the_quit() {
    let mut app = App::new(None);
    app.set_changed(true);
    app.key(ctrl('q'));
    app.key(press(KeyCode::Tab));
    assert_eq!(app.mode(), Mode::Run);
    app.key(ctrl('q'));
    assert!(!app.should_quit());
}

#[test]
fn the_quit_question_shows_in_full_at_the_smallest_width() {
    let mut app = App::new(Some("hello.bbas".to_string()));
    app.set_changed(true);
    app.key(ctrl('q'));
    let status = row_text(&draw(&app, 60, 16), 15);
    assert!(
        status.contains("Not saved. Press ^Q again to quit"),
        "{status:?}"
    );
}

#[test]
fn changed_mark_shows_in_the_status_line() {
    let mut app = App::new(Some("hello.bbas".to_string()));
    app.set_changed(true);
    let status = row_text(&draw(&app, 80, 24), 23);
    assert!(status.contains("hello.bbas *"), "{status:?}");
}

#[test]
fn ctrl_r_is_not_available_yet() {
    // Issue #172 gives Ctrl-S and Ctrl-O their action.
    let mut app = App::new(None);
    app.key(ctrl('r'));
    assert_eq!(app.message(), "not available yet");
    assert!(!app.should_quit());
    let status = row_text(&draw(&app, 80, 24), 23);
    assert!(status.contains("not available yet"), "{status:?}");
}

/// Run the binary with the arguments and the input on a pipe. Give the exit
/// code, stdout and stderr. Kill the binary after 5 seconds.
fn run_binary_with_args(args: &[&str], input: &str) -> (Option<i32>, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_bbc-basic-interpreter"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("binary starts");
    let mut stdout = child.stdout.take().expect("stdout is piped");
    let mut stderr = child.stderr.take().expect("stderr is piped");
    let out_reader = thread::spawn(move || {
        let mut text = String::new();
        let _ = stdout.read_to_string(&mut text);
        text
    });
    let err_reader = thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });
    // The binary can end before it reads the input, so ignore a write error.
    let _ = child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(input.as_bytes());
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait().expect("child status") {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("the binary ran for more than 5 seconds");
        }
        thread::sleep(Duration::from_millis(20));
    };
    (
        status.code(),
        out_reader.join().expect("stdout reader"),
        err_reader.join().expect("stderr reader"),
    )
}

#[test]
fn ide_flag_with_a_pipe_gives_a_clear_message_and_code_2() {
    let (code, stdout, stderr) = run_binary_with_args(&["--ide"], "EXIT\n");
    assert_eq!(code, Some(2));
    assert_eq!(stderr, "The IDE needs a terminal\n");
    assert!(!stdout.contains("\x1b[?1049h"), "{stdout:?}");
}

#[test]
fn ide_flag_with_a_file_and_a_pipe_gives_the_same_message() {
    let (code, _, stderr) = run_binary_with_args(&["--ide", "hello.bbas"], "EXIT\n");
    assert_eq!(code, Some(2));
    assert_eq!(stderr, "The IDE needs a terminal\n");
}

#[test]
fn unknown_argument_gives_the_usage_line_and_code_2() {
    for args in [&["--foo"][..], &["--ide", "a.bbas", "b.bbas"][..]] {
        let (code, stdout, stderr) = run_binary_with_args(args, "EXIT\n");
        assert_eq!(code, Some(2), "{args:?}");
        assert_eq!(stderr, "Usage: bbc-basic-interpreter [--ide [file]]\n");
        assert_eq!(stdout, "", "{args:?}");
    }
}

#[test]
fn no_argument_gives_the_prompt_mode() {
    let (code, stdout, _) = run_binary_with_args(&[], "PRINT 6*7\nEXIT\n");
    assert_eq!(code, Some(0));
    assert!(
        stdout.starts_with("BBC BASIC Interpreter v0.1.0"),
        "{stdout:?}"
    );
    assert!(stdout.contains("> 42\n"), "{stdout:?}");
    assert!(stdout.contains("Goodbye!"), "{stdout:?}");
}
