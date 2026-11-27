//! Issue #171: the run mode of the IDE. Ctrl-R runs the program on a worker
//! thread. The pane shows its text on a text screen, gives it the typed keys
//! and stops it at Escape. After the run, the pane has the prompt `>`.
//! The tests call `App::key`, wait for the worker with a time limit, and
//! draw with the test backend of ratatui.

use crate::common::run_binary_read_first;
use bbc_basic_interpreter::ui::{App, Mode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::Terminal;
use std::time::{Duration, Instant};

/// The time limit for a run that must end.
const LIMIT: Duration = Duration::from_secs(5);

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

/// An app with the program text in the editor.
fn app_with(text: &str) -> App {
    let mut app = App::new(None);
    app.paste(text);
    app
}

fn draw(app: &App) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("test terminal");
    terminal.draw(|frame| app.draw(frame)).expect("draw");
    terminal.backend().buffer().clone()
}

fn row_text(buffer: &Buffer, row: u16) -> String {
    (0..buffer.area.width)
        .map(|x| buffer[(x, row)].symbol())
        .collect()
}

/// The start text of the pane (issue #174), before the first prompt.
const BANNER: [&str; 4] = ["BBC Computer 32K", "", "BASIC", ""];

/// The rows of the pane (the screen with no tab bar and no status line),
/// with no spaces at the end, and with no empty rows at the end. The banner
/// at the top is left out.
fn pane_rows(app: &App) -> Vec<String> {
    let buffer = draw(app);
    let mut rows: Vec<String> = (1..buffer.area.height - 1)
        .map(|row| row_text(&buffer, row).trim_end().to_string())
        .collect();
    if rows.starts_with(&BANNER.map(String::from)) {
        rows.drain(..BANNER.len());
    }
    while rows.last().is_some_and(String::is_empty) {
        rows.pop();
    }
    rows
}

fn status_row(app: &App) -> String {
    let buffer = draw(app);
    row_text(&buffer, buffer.area.height - 1)
}

/// Press Ctrl-R and wait for the end of the run.
fn run(app: &mut App) {
    app.key(ctrl('r'));
    assert!(app.wait_for_run(LIMIT), "the run did not end");
}

/// Wait until a pane row has the text, with a time limit.
fn wait_for_row(app: &mut App, text: &str) {
    let deadline = Instant::now() + LIMIT;
    while !pane_rows(app).iter().any(|row| row.contains(text)) {
        assert!(Instant::now() < deadline, "no row with {text:?}");
        app.wait_for_run(Duration::from_millis(10));
    }
}

#[test]
fn a_program_that_prints_3_lines_shows_them() {
    let mut app = app_with("10 PRINT \"ONE\"\n20 PRINT \"TWO\"\n30 PRINT \"THREE\"");
    run(&mut app);
    assert_eq!(app.mode(), Mode::Run);
    assert_eq!(pane_rows(&app), ["> RUN", "ONE", "TWO", "THREE", ">"]);
    assert!(status_row(&app).contains("ready"), "{:?}", status_row(&app));
}

#[test]
fn a_program_of_100_lines_scrolls_and_page_up_shows_the_history() {
    let mut app = app_with("10 FOR I% = 1 TO 100\n20 PRINT I%\n30 NEXT");
    run(&mut app);
    // 102 lines: "> RUN", 1 to 100 and the prompt. The pane has 22 rows.
    let rows = pane_rows(&app);
    assert_eq!(rows.len(), 22);
    assert_eq!(rows[0].trim(), "80");
    assert_eq!(rows[20].trim(), "100");
    assert_eq!(rows[21], ">");
    app.key(press(KeyCode::PageUp));
    assert_eq!(pane_rows(&app)[0].trim(), "58");
    app.key(press(KeyCode::PageDown));
    assert_eq!(pane_rows(&app)[0].trim(), "80");
}

#[test]
fn input_takes_the_typed_keys_and_enter() {
    let mut app = app_with("10 INPUT A$\n20 PRINT \"Hi \";A$");
    app.key(ctrl('r'));
    type_text(&mut app, "BOB");
    app.key(press(KeyCode::Enter));
    assert!(app.wait_for_run(LIMIT));
    assert_eq!(pane_rows(&app), ["> RUN", "?BOB", "Hi BOB", ">"]);
}

#[test]
fn input_line_takes_a_line_with_commas() {
    let mut app = app_with("10 INPUT LINE A$\n20 PRINT A$");
    app.key(ctrl('r'));
    type_text(&mut app, "A, B");
    app.key(press(KeyCode::Enter));
    assert!(app.wait_for_run(LIMIT));
    assert_eq!(pane_rows(&app), ["> RUN", "?A, B", "A, B", ">"]);
}

#[test]
fn backspace_removes_a_typed_character_of_the_reply() {
    let mut app = app_with("10 INPUT A$\n20 PRINT A$");
    app.key(ctrl('r'));
    type_text(&mut app, "AX");
    app.key(press(KeyCode::Backspace));
    type_text(&mut app, "B");
    app.key(press(KeyCode::Enter));
    assert!(app.wait_for_run(LIMIT));
    assert_eq!(pane_rows(&app), ["> RUN", "?AB", "AB", ">"]);
}

#[test]
fn get_dollar_takes_one_key_with_no_enter() {
    let mut app = app_with("10 A$ = GET$\n20 PRINT \"Key \";A$");
    app.key(ctrl('r'));
    type_text(&mut app, "x");
    assert!(app.wait_for_run(LIMIT));
    assert_eq!(pane_rows(&app), ["> RUN", "Key x", ">"]);
}

#[test]
fn get_and_inkey_take_the_key_codes() {
    let mut app = app_with("10 A% = GET\n20 B% = INKEY(500)\n30 PRINT A%;\" \";B%");
    app.key(ctrl('r'));
    type_text(&mut app, "AB");
    assert!(app.wait_for_run(LIMIT));
    assert_eq!(pane_rows(&app), ["> RUN", "65 66", ">"]);
}

#[test]
fn inkey_with_no_key_gives_minus_1_after_the_time_limit() {
    let mut app = app_with("10 PRINT INKEY(1)");
    run(&mut app);
    assert_eq!(pane_rows(&app)[1].trim(), "-1");
}

#[test]
fn escape_stops_a_loop_with_no_end_and_the_frame_answers_keys() {
    let mut app = app_with("10 REPEAT: UNTIL FALSE");
    app.key(ctrl('r'));
    // The run does not end by itself
    assert!(!app.wait_for_run(Duration::from_millis(100)));
    assert!(app.is_running());
    // The frame answers the keys during the run
    app.key(press(KeyCode::Tab));
    assert_eq!(app.mode(), Mode::Files);
    app.key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
    assert_eq!(app.mode(), Mode::Run);
    assert!(
        status_row(&app).contains("running"),
        "{:?}",
        status_row(&app)
    );
    app.key(press(KeyCode::Esc));
    assert!(app.wait_for_run(LIMIT));
    assert_eq!(pane_rows(&app), ["> RUN", "Error: Escape at line 10", ">"]);
    assert!(status_row(&app).contains("ready"), "{:?}", status_row(&app));
}

#[test]
fn on_error_traps_escape_with_err_17() {
    let mut app =
        app_with("10 ON ERROR PRINT \"Trapped \";ERR;\" at \";ERL: END\n20 REPEAT\n30 UNTIL FALSE");
    app.key(ctrl('r'));
    assert!(!app.wait_for_run(Duration::from_millis(50)));
    app.key(press(KeyCode::Esc));
    assert!(app.wait_for_run(LIMIT));
    let rows = pane_rows(&app);
    assert_eq!(rows.len(), 3, "{rows:?}");
    assert!(
        rows[1] == "Trapped 17 at 20" || rows[1] == "Trapped 17 at 30",
        "{rows:?}"
    );
}

#[test]
fn escape_stops_a_program_that_waits_in_input() {
    let mut app = app_with("10 INPUT A$\n20 PRINT \"After\"");
    app.key(ctrl('r'));
    wait_for_row(&mut app, "?");
    assert!(app.is_running());
    app.key(press(KeyCode::Esc));
    assert!(app.wait_for_run(LIMIT));
    assert_eq!(
        pane_rows(&app),
        ["> RUN", "?", "Error: Escape at line 10", ">"]
    );
}

#[test]
fn escape_stops_a_program_that_waits_in_get() {
    let mut app = app_with("10 A$ = GET$\n20 PRINT \"After\"");
    app.key(ctrl('r'));
    assert!(!app.wait_for_run(Duration::from_millis(50)));
    app.key(press(KeyCode::Esc));
    assert!(app.wait_for_run(LIMIT));
    assert_eq!(pane_rows(&app), ["> RUN", "Error: Escape at line 10", ">"]);
}

#[test]
fn escape_stops_a_loop_in_a_function_body() {
    let mut app = app_with("10 PRINT FNx\n20 END\n30 DEF FNx\n40 REPEAT: UNTIL FALSE\n50 = 1");
    app.key(ctrl('r'));
    assert!(!app.wait_for_run(Duration::from_millis(50)));
    app.key(press(KeyCode::Esc));
    assert!(app.wait_for_run(LIMIT));
    assert_eq!(pane_rows(&app), ["> RUN", "Error: Escape at line 40", ">"]);
}

#[test]
fn an_error_shows_its_text_and_enter_goes_to_its_line_in_edit() {
    let mut app = app_with("10 PRINT 1\n20 X = 1 / 0\n30 PRINT 3");
    run(&mut app);
    assert_eq!(
        pane_rows(&app),
        ["> RUN", "1", "Error: Division by zero at line 20", ">"]
    );
    app.key(press(KeyCode::Enter));
    // The worker says that the line is empty: a short wait
    assert!(app.wait_for_run(LIMIT));
    assert_eq!(app.mode(), Mode::Edit);
    assert_eq!(app.cursor(), (1, 0));
}

#[test]
fn enter_after_a_typed_line_does_not_go_to_edit() {
    let mut app = app_with("10 X = 1 / 0");
    run(&mut app);
    type_text(&mut app, "PRINT 7");
    app.key(press(KeyCode::Enter));
    assert!(app.wait_for_run(LIMIT));
    assert_eq!(app.mode(), Mode::Run);
    assert_eq!(pane_rows(&app).last().map(String::as_str), Some(">"));
    // No error now: Enter on the empty line runs the empty line
    app.key(press(KeyCode::Enter));
    assert!(app.wait_for_run(LIMIT));
    assert_eq!(app.mode(), Mode::Run);
}

#[test]
fn a_typed_print_at_the_prompt_of_the_pane_prints_2() {
    let mut app = App::new(None);
    app.key(press(KeyCode::Tab));
    assert_eq!(app.mode(), Mode::Run);
    type_text(&mut app, "PRINT 1 + 1");
    app.key(press(KeyCode::Enter));
    assert!(app.wait_for_run(LIMIT));
    assert_eq!(pane_rows(&app), ["> PRINT 1 + 1", "2", ">"]);
}

#[test]
fn the_prompt_of_the_pane_has_the_commands_of_the_prompt_mode() {
    let mut app = app_with("10 PRINT \"A\"\n20 END");
    app.key(press(KeyCode::Tab));
    type_text(&mut app, "LIST");
    app.key(press(KeyCode::Enter));
    assert!(app.wait_for_run(LIMIT));
    type_text(&mut app, "RUN");
    app.key(press(KeyCode::Enter));
    assert!(app.wait_for_run(LIMIT));
    assert_eq!(
        pane_rows(&app),
        ["> LIST", "10 PRINT \"A\"", "20 END", "> RUN", "A", ">"]
    );
}

#[test]
fn new_at_the_prompt_of_the_pane_does_not_change_the_document() {
    let mut app = app_with("10 PRINT \"A\"");
    run(&mut app);
    type_text(&mut app, "NEW");
    app.key(press(KeyCode::Enter));
    assert!(app.wait_for_run(LIMIT));
    assert_eq!(app.document().lines(), ["10 PRINT \"A\""]);
    assert_eq!(
        app.message(),
        "The RUN program changed. The editor text is the source"
    );
    // K3: the next typed line uses the editor text again
    type_text(&mut app, "LIST");
    app.key(press(KeyCode::Enter));
    assert!(app.wait_for_run(LIMIT));
    let rows = pane_rows(&app);
    assert_eq!(rows[rows.len() - 3..], ["> LIST", "10 PRINT \"A\"", ">"]);
}

#[test]
fn a_typed_program_line_does_not_stay_after_the_next_typed_line() {
    // K3: the editor text is the source for each typed line
    let mut app = app_with("10 PRINT \"A\"");
    run(&mut app);
    type_text(&mut app, "20 PRINT \"B\"");
    app.key(press(KeyCode::Enter));
    assert!(app.wait_for_run(LIMIT));
    assert_eq!(
        app.message(),
        "The RUN program changed. The editor text is the source"
    );
    type_text(&mut app, "RUN");
    app.key(press(KeyCode::Enter));
    assert!(app.wait_for_run(LIMIT));
    let rows = pane_rows(&app);
    assert_eq!(rows[rows.len() - 3..], ["> RUN", "A", ">"]);
}

#[test]
fn a_typed_line_with_a_fault_in_the_text_is_refused_and_goes_to_the_fault() {
    // K1: a typed line must not run or list a part of the program
    for typed in ["RUN", "LIST", "GOTO 30", "PRINT 2"] {
        let mut app = app_with("10 PRINT 1\n20 PRNT 2 +\n30 PRINT 3");
        app.key(press(KeyCode::Tab));
        type_text(&mut app, typed);
        app.key(press(KeyCode::Enter));
        assert!(app.wait_for_run(LIMIT), "{typed}");
        assert_eq!(app.mode(), Mode::Edit, "{typed}");
        assert_eq!(app.cursor(), (1, 0), "{typed}");
        assert_eq!(app.message(), app.marks()[0].message, "{typed}");
        app.key(press(KeyCode::Tab));
        let rows = pane_rows(&app);
        assert!(!rows.iter().any(|row| row == "1" || row == "3"), "{rows:?}");
    }
}

#[test]
fn enter_after_an_error_runs_a_line_typed_during_the_run() {
    // K4: LIST typed during the run is not an empty line
    let mut app = app_with("10 A$ = GET$\n20 X = 1 / 0");
    app.key(ctrl('r'));
    type_text(&mut app, "xLIST");
    assert!(app.wait_for_run(LIMIT));
    wait_for_row(&mut app, "> LIST");
    app.key(press(KeyCode::Enter));
    assert!(app.wait_for_run(LIMIT));
    assert_eq!(app.mode(), Mode::Run);
    let rows = pane_rows(&app);
    assert_eq!(
        rows[rows.len() - 4..],
        ["> LIST", "10 A$ = GET$", "20 X = 1 / 0", ">"]
    );
}

#[test]
fn enter_after_an_error_of_another_program_stays_in_run() {
    // K4: the error of a CHAIN is at a line of the file, not of the editor
    let path = std::env::temp_dir().join(format!("issue_171_chain_{}.bbas", std::process::id()));
    std::fs::write(&path, "10 PRINT \"C\"\n20 X = 1 / 0\n").expect("write the file");
    let mut app = app_with("10 PRINT \"A\"\n20 PRINT \"B\"");
    app.key(press(KeyCode::Tab));
    type_text(&mut app, &format!("CHAIN \"{}\"", path.display()));
    app.key(press(KeyCode::Enter));
    assert!(app.wait_for_run(LIMIT));
    let _ = std::fs::remove_file(&path);
    assert!(
        pane_rows(&app).contains(&"Error: Division by zero at line 20".to_string()),
        "{:?}",
        pane_rows(&app)
    );
    app.key(press(KeyCode::Enter));
    assert!(app.wait_for_run(LIMIT));
    assert_eq!(app.mode(), Mode::Run);
}

#[test]
fn a_paste_in_the_run_mode_types_the_text() {
    // K5
    let mut app = App::new(None);
    app.key(press(KeyCode::Tab));
    app.paste("PRINT 3\nPRINT 4\n");
    assert!(app.wait_for_run(LIMIT));
    wait_for_row(&mut app, "4");
    assert!(app.wait_for_run(LIMIT));
    assert_eq!(pane_rows(&app), ["> PRINT 3", "3", "> PRINT 4", "4", ">"]);
}

#[test]
fn backspace_goes_back_over_the_wrap_of_a_long_line() {
    // K6: the prompt and 85 characters wrap at column 80
    let mut app = App::new(None);
    app.key(press(KeyCode::Tab));
    let _ = pane_rows(&app);
    type_text(&mut app, &"a".repeat(85));
    for _ in 0..10 {
        app.key(press(KeyCode::Backspace));
    }
    type_text(&mut app, "b");
    let deadline = Instant::now() + LIMIT;
    loop {
        let rows = pane_rows(&app);
        let text: String = rows.concat();
        if text.contains('b') {
            assert_eq!(text.matches('a').count(), 75, "{rows:?}");
            assert_eq!(rows[0], format!("> {}", "a".repeat(75)) + "b");
            break;
        }
        assert!(Instant::now() < deadline, "{rows:?}");
        app.wait_for_run(Duration::from_millis(10));
        app.tick();
    }
}

#[test]
fn escape_stops_a_run_that_started_from_a_line_typed_during_the_run() {
    // K2: A% stays after RUN, so the second run is the loop with no end
    let mut app = app_with("10 IF A% = 0 THEN A% = 1: END\n20 REPEAT: UNTIL FALSE");
    app.key(ctrl('r'));
    type_text(&mut app, "RUN");
    app.key(press(KeyCode::Enter));
    let deadline = Instant::now() + LIMIT;
    while pane_rows(&app).iter().filter(|row| *row == "> RUN").count() < 2 || !app.is_running() {
        assert!(Instant::now() < deadline, "{:?}", pane_rows(&app));
        app.wait_for_run(Duration::from_millis(10));
    }
    app.key(press(KeyCode::Esc));
    assert!(app.wait_for_run(LIMIT));
    let rows = pane_rows(&app);
    assert_eq!(rows[rows.len() - 2..], ["Error: Escape at line 20", ">"]);
}

#[test]
fn a_short_form_at_the_prompt_of_the_pane_is_the_full_keyword() {
    let mut app = app_with("10 PRINT 1");
    app.key(press(KeyCode::Tab));
    type_text(&mut app, "L.");
    app.key(press(KeyCode::Enter));
    assert!(app.wait_for_run(LIMIT));
    assert_eq!(pane_rows(&app), ["> L.", "10 PRINT 1", ">"]);
}

#[test]
fn cls_clears_the_pane() {
    let mut app = app_with("10 PRINT \"A\"\n20 CLS\n30 PRINT \"B\"");
    run(&mut app);
    assert_eq!(pane_rows(&app), ["B", ">"]);
}

#[test]
fn ctrl_r_with_a_fault_goes_to_the_line_in_edit_and_does_not_run() {
    let mut app = app_with("10 PRINT 1\n20 PRINT (\n30 END");
    app.key(press(KeyCode::Tab));
    app.key(ctrl('r'));
    assert_eq!(app.mode(), Mode::Edit);
    assert_eq!(app.cursor().0, 1);
    assert!(!app.is_running());
    assert_eq!(app.marks().len(), 1);
    assert_eq!(app.message(), app.marks()[0].message);
}

#[test]
fn ctrl_r_from_the_files_mode_runs_and_changes_to_run() {
    let mut app = app_with("10 PRINT \"F\"");
    app.key(press(KeyCode::Tab));
    app.key(press(KeyCode::Tab));
    assert_eq!(app.mode(), Mode::Files);
    run(&mut app);
    assert_eq!(app.mode(), Mode::Run);
    assert_eq!(pane_rows(&app), ["> RUN", "F", ">"]);
}

#[test]
fn a_second_ctrl_r_runs_the_changed_text() {
    let mut app = app_with("10 PRINT \"A\"");
    run(&mut app);
    app.key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
    app.key(press(KeyCode::End));
    app.key(press(KeyCode::Backspace));
    app.key(press(KeyCode::Backspace));
    type_text(&mut app, "B\"");
    run(&mut app);
    assert_eq!(pane_rows(&app), ["> RUN", "A", "> RUN", "B", ">"]);
}

#[test]
fn a_typed_run_uses_the_text_of_the_editor() {
    let mut app = app_with("10 PRINT \"A\"");
    run(&mut app);
    app.key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
    app.paste("\n20 PRINT \"C\"");
    app.key(press(KeyCode::Tab));
    type_text(&mut app, "RUN");
    app.key(press(KeyCode::Enter));
    assert!(app.wait_for_run(LIMIT));
    assert_eq!(pane_rows(&app), ["> RUN", "A", "> RUN", "A", "C", ">"]);
}

#[test]
fn stop_ends_a_worker_whose_program_traps_escape() {
    let mut app = app_with("10 ON ERROR GOTO 20\n20 GOTO 20");
    app.key(ctrl('r'));
    // Line 10 sets the handler before the Escape
    assert!(!app.wait_for_run(Duration::from_millis(50)));
    app.key(press(KeyCode::Esc));
    assert!(!app.wait_for_run(Duration::from_millis(50)));
    assert!(app.is_running());
    assert!(app.stop_worker(LIMIT), "the worker did not stop");
}

#[test]
fn a_deep_recursion_runs_on_the_large_stack_of_the_worker() {
    // 250 levels of FN calls overflow the 2 MiB stack of a test thread
    let mut app = app_with(
        "10 PRINT FNd(250)\n20 END\n30 DEF FNd(N%)\n40 IF N% = 0 THEN = 0\n50 = 1 + FNd(N% - 1)",
    );
    run(&mut app);
    assert_eq!(pane_rows(&app)[1], "250");
}

#[test]
fn the_prompt_mode_with_no_flag_is_unchanged() {
    let output = run_binary_read_first("10 PRINT \"HI\"\nRUN\nLIST\nX = 1 / 0\nNEW\nLIST\nEXIT\n");
    assert!(
        output.ends_with(
            "> > HI\n> 10 PRINT \"HI\"\n> Error: Division by zero\n> Program cleared\n> No program\n> Goodbye!\n"
        ),
        "{output:?}"
    );
}
