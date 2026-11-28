//! Issue #170: the edit mode of the IDE. A full-screen editor for the program
//! text, with the line number in a dim column, the movement keys, Enter with
//! the next free line number, and a mark at a line with a fault.
//! The key tests call `App::key` and read the document and the cursor. The
//! screen tests use the test backend of ratatui.

use bbc_basic_interpreter::tokenizer::detokenize;
use bbc_basic_interpreter::ui::{App, Mode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::Color;
use ratatui::Terminal;

fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl_key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::CONTROL)
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        app.key(press(KeyCode::Char(c)));
    }
}

fn keys(app: &mut App, code: KeyCode, count: usize) {
    for _ in 0..count {
        app.key(press(code));
    }
}

/// Type each line, with Enter between. Enter gives the next line number, so
/// each line after the first holds only the statements.
fn type_lines(app: &mut App, first: &str, rest: &[&str]) {
    type_text(app, first);
    for line in rest {
        app.key(press(KeyCode::Enter));
        type_text(app, line);
    }
}

/// An app with one line for each text. Each text holds its line number:
/// Enter gives a number, and Backspace removes it before the text is typed.
fn app_with_lines(lines: &[&str]) -> App {
    let mut app = App::new(None);
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            app.key(press(KeyCode::End));
            app.key(press(KeyCode::Enter));
            let number_len = app.cursor().1;
            keys(&mut app, KeyCode::Backspace, number_len);
        }
        type_text(&mut app, line);
    }
    app
}

fn lines(app: &App) -> Vec<String> {
    app.document().lines().to_vec()
}

fn draw(app: &App, width: u16, height: u16) -> (Buffer, (u16, u16)) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
    terminal.draw(|frame| app.draw(frame)).expect("draw");
    let cursor = terminal.get_cursor_position().expect("cursor");
    (terminal.backend().buffer().clone(), (cursor.x, cursor.y))
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

#[test]
fn type_three_lines_and_read_the_screen() {
    let mut app = App::new(None);
    type_lines(&mut app, "10 PRINT \"A\"", &["PRINT \"B\"", "END"]);
    assert_eq!(lines(&app), ["10 PRINT \"A\"", "20 PRINT \"B\"", "30 END"]);
    let (buffer, _) = draw(&app, 80, 24);
    assert_eq!(row_text(&buffer, 1).trim_end(), "   10  PRINT \"A\"");
    assert_eq!(row_text(&buffer, 2).trim_end(), "   20  PRINT \"B\"");
    assert_eq!(row_text(&buffer, 3).trim_end(), "   30  END");
}

#[test]
fn the_line_number_has_a_dim_colour_and_the_text_is_white() {
    let app = app_with_lines(&["10 END"]);
    let (buffer, _) = draw(&app, 80, 24);
    assert_eq!(buffer[(3, 1)].symbol(), "1");
    assert_eq!(buffer[(3, 1)].fg, Color::DarkGray);
    assert_eq!(buffer[(7, 1)].symbol(), "E");
    assert_eq!(buffer[(7, 1)].fg, Color::White);
    assert_eq!(buffer[(7, 1)].bg, Color::Black);
}

#[test]
fn the_screen_cursor_follows_the_text_cursor() {
    let mut app = app_with_lines(&["10 PRINT 1"]);
    // After the "P": the number column is 5 wide, then 2 spaces.
    keys(&mut app, KeyCode::Left, 6);
    assert_eq!(app.cursor(), (0, 4));
    assert_eq!(draw(&app, 80, 24).1, (8, 1));
    // In the line number
    app.key(press(KeyCode::Home));
    assert_eq!(draw(&app, 80, 24).1, (3, 1));
}

#[test]
fn enter_gives_the_number_of_the_line_plus_10() {
    let mut app = App::new(None);
    type_text(&mut app, "10 REM");
    app.key(press(KeyCode::Enter));
    assert_eq!(lines(&app), ["10 REM", "20 "]);
    assert_eq!(app.cursor(), (1, 3));
}

#[test]
fn enter_gives_the_middle_of_the_gap_to_the_next_line() {
    let mut app = app_with_lines(&["10 REM A", "20 REM B"]);
    app.key(press(KeyCode::Up));
    app.key(press(KeyCode::End));
    app.key(press(KeyCode::Enter));
    assert_eq!(lines(&app), ["10 REM A", "15 ", "20 REM B"]);
    assert_eq!(app.cursor(), (1, 3));
}

#[test]
fn enter_gives_no_number_if_the_gap_has_no_free_number() {
    let mut app = app_with_lines(&["10 REM A", "11 REM B"]);
    app.key(press(KeyCode::Up));
    app.key(press(KeyCode::End));
    app.key(press(KeyCode::Enter));
    assert_eq!(lines(&app), ["10 REM A", "", "11 REM B"]);
    assert_eq!(app.cursor(), (1, 0));
}

#[test]
fn enter_in_the_middle_of_a_line_splits_it() {
    let mut app = app_with_lines(&["10 PRINT 1:PRINT 2"]);
    keys(&mut app, KeyCode::Left, 8);
    app.key(press(KeyCode::Enter));
    assert_eq!(lines(&app), ["10 PRINT 1", "20 :PRINT 2"]);
    assert_eq!(app.cursor(), (1, 3));
}

#[test]
fn enter_at_the_start_of_a_line_puts_an_empty_line_above() {
    let mut app = app_with_lines(&["10 END"]);
    app.key(press(KeyCode::Home));
    app.key(press(KeyCode::Enter));
    assert_eq!(lines(&app), ["", "10 END"]);
    assert_eq!(app.cursor(), (1, 0));
}

#[test]
fn typed_characters_are_inserted_at_the_cursor() {
    let mut app = app_with_lines(&["10 PRINT 12"]);
    app.key(press(KeyCode::Left));
    type_text(&mut app, "5");
    assert_eq!(lines(&app), ["10 PRINT 152"]);
    assert_eq!(app.cursor(), (0, 11));
    app.key(press(KeyCode::Home));
    type_text(&mut app, "1");
    assert_eq!(lines(&app), ["110 PRINT 152"]);
}

#[test]
fn backspace_deletes_the_character_before_the_cursor() {
    let mut app = app_with_lines(&["10 PRINT 123"]);
    app.key(press(KeyCode::Left));
    app.key(press(KeyCode::Backspace));
    assert_eq!(lines(&app), ["10 PRINT 13"]);
    assert_eq!(app.cursor(), (0, 10));
}

#[test]
fn backspace_at_the_start_of_a_line_joins_it_to_the_line_before() {
    let mut app = app_with_lines(&["10 A=1", "20 B=2"]);
    app.key(press(KeyCode::Home));
    app.key(press(KeyCode::Backspace));
    assert_eq!(lines(&app), ["10 A=120 B=2"]);
    assert_eq!(app.cursor(), (0, 6));
}

#[test]
fn backspace_at_the_start_of_the_text_does_nothing() {
    let mut app = app_with_lines(&["10 END"]);
    app.key(press(KeyCode::Home));
    app.key(press(KeyCode::Backspace));
    assert_eq!(lines(&app), ["10 END"]);
    assert_eq!(app.cursor(), (0, 0));
}

#[test]
fn delete_removes_the_character_at_the_cursor() {
    let mut app = app_with_lines(&["10 PRINT 123"]);
    keys(&mut app, KeyCode::Left, 2);
    app.key(press(KeyCode::Delete));
    assert_eq!(lines(&app), ["10 PRINT 13"]);
    assert_eq!(app.cursor(), (0, 10));
}

#[test]
fn delete_at_the_end_of_a_line_joins_the_next_line() {
    let mut app = app_with_lines(&["10 A=1", "20 B=2"]);
    app.key(press(KeyCode::Up));
    app.key(press(KeyCode::End));
    app.key(press(KeyCode::Delete));
    assert_eq!(lines(&app), ["10 A=120 B=2"]);
    assert_eq!(app.cursor(), (0, 6));
}

#[test]
fn delete_at_the_end_of_the_text_does_nothing() {
    let mut app = app_with_lines(&["10 END"]);
    app.key(press(KeyCode::Delete));
    assert_eq!(lines(&app), ["10 END"]);
    assert_eq!(app.cursor(), (0, 6));
}

#[test]
fn left_and_right_move_in_a_line_and_across_lines() {
    let mut app = app_with_lines(&["10 A=1", "20 B=2"]);
    // At the end of the text, Right does nothing.
    app.key(press(KeyCode::Right));
    assert_eq!(app.cursor(), (1, 6));
    // In the middle
    app.key(press(KeyCode::Left));
    assert_eq!(app.cursor(), (1, 5));
    // At the start of a line, Left goes to the end of the line before.
    app.key(press(KeyCode::Home));
    app.key(press(KeyCode::Left));
    assert_eq!(app.cursor(), (0, 6));
    // At the end of a line, Right goes to the start of the next line.
    app.key(press(KeyCode::Right));
    assert_eq!(app.cursor(), (1, 0));
    // At the start of the text, Left does nothing.
    app.key(ctrl_key(KeyCode::Home));
    app.key(press(KeyCode::Left));
    assert_eq!(app.cursor(), (0, 0));
    app.key(press(KeyCode::Right));
    assert_eq!(app.cursor(), (0, 1));
}

#[test]
fn up_and_down_keep_the_column_in_the_line() {
    let mut app = app_with_lines(&["10 PRINT 12345", "20 END", "30 PRINT 1"]);
    // On the first line, Up does nothing.
    app.key(ctrl_key(KeyCode::Home));
    app.key(press(KeyCode::Up));
    assert_eq!(app.cursor(), (0, 0));
    // In the middle of a line
    keys(&mut app, KeyCode::Right, 4);
    app.key(press(KeyCode::Down));
    assert_eq!(app.cursor(), (1, 4));
    // At the end of a long line, Down goes to the end of a short line.
    app.key(press(KeyCode::Up));
    app.key(press(KeyCode::End));
    app.key(press(KeyCode::Down));
    assert_eq!(app.cursor(), (1, 6));
    app.key(press(KeyCode::Down));
    assert_eq!(app.cursor(), (2, 6));
    // On the last line, Down does nothing.
    app.key(press(KeyCode::Down));
    assert_eq!(app.cursor(), (2, 6));
}

#[test]
fn home_end_ctrl_home_and_ctrl_end() {
    let mut app = app_with_lines(&["10 A=1", "20 B=22", "30 C=333"]);
    app.key(press(KeyCode::Up));
    app.key(press(KeyCode::Home));
    assert_eq!(app.cursor(), (1, 0));
    app.key(press(KeyCode::Home));
    assert_eq!(app.cursor(), (1, 0));
    app.key(press(KeyCode::End));
    assert_eq!(app.cursor(), (1, 7));
    app.key(press(KeyCode::End));
    assert_eq!(app.cursor(), (1, 7));
    app.key(ctrl_key(KeyCode::Home));
    assert_eq!(app.cursor(), (0, 0));
    app.key(ctrl_key(KeyCode::End));
    assert_eq!(app.cursor(), (2, 8));
}

/// An app with 200 lines, from "10 PRINT 10" to "2000 PRINT 2000".
fn app_with_200_lines() -> App {
    let mut app = App::new(None);
    type_text(&mut app, "10 PRINT 10");
    for n in 2..=200 {
        app.key(press(KeyCode::Enter));
        type_text(&mut app, &format!("PRINT {}", n * 10));
    }
    app
}

#[test]
fn page_up_and_page_down_move_one_page_of_lines() {
    let mut app = app_with_200_lines();
    let (_, _) = draw(&app, 80, 24);
    // The pane is 22 rows high: 24 rows less the tab bar and the status line.
    app.key(press(KeyCode::PageUp));
    assert_eq!(app.cursor().0, 199 - 22);
    app.key(ctrl_key(KeyCode::Home));
    app.key(press(KeyCode::PageUp));
    assert_eq!(app.cursor(), (0, 0));
    app.key(press(KeyCode::PageDown));
    assert_eq!(app.cursor().0, 22);
    app.key(ctrl_key(KeyCode::End));
    app.key(press(KeyCode::PageDown));
    assert_eq!(app.cursor().0, 199);
}

#[test]
fn a_text_of_200_lines_scrolls() {
    let mut app = app_with_200_lines();
    assert_eq!(app.document().lines().len(), 200);
    let (buffer, cursor) = draw(&app, 80, 24);
    let text = screen_text(&buffer);
    assert!(text.contains(" 2000  PRINT 2000"), "{text}");
    assert!(
        !(1..23).any(|row| row_text(&buffer, row).trim_end() == "   10  PRINT 10"),
        "{text}"
    );
    assert_eq!(row_text(&buffer, 22).trim_end(), " 2000  PRINT 2000");
    assert_eq!(cursor.1, 22);
    app.key(ctrl_key(KeyCode::Home));
    let (buffer, cursor) = draw(&app, 80, 24);
    assert_eq!(row_text(&buffer, 1).trim_end(), "   10  PRINT 10");
    assert_eq!(cursor, (3, 1));
}

#[test]
fn a_long_line_scrolls_to_the_right_and_back() {
    let long = format!("10 REM {}", "x".repeat(100) + "END");
    let mut app = app_with_lines(&[&long]);
    let (buffer, cursor) = draw(&app, 80, 24);
    let row = row_text(&buffer, 1);
    assert!(row.contains("xEND"), "{row:?}");
    assert!(!row.contains("REM"), "{row:?}");
    assert!(row.starts_with("   10  "), "{row:?}");
    assert_eq!(cursor, (79, 1));
    app.key(press(KeyCode::Home));
    let (buffer, _) = draw(&app, 80, 24);
    let row = row_text(&buffer, 1);
    assert!(row.starts_with("   10  REM xxx"), "{row:?}");
}

#[test]
fn a_line_with_a_syntax_error_gets_its_mark_and_a_correct_line_gets_none() {
    let mut app = App::new(None);
    type_lines(&mut app, "10 PRINT 1", &["PRINT 2 \\", "END"]);
    let marks = app.marks();
    assert_eq!(marks.len(), 1, "{marks:?}");
    assert_eq!(marks[0].index, 1);
    assert_eq!(
        marks[0].message,
        "Syntax error at line 20: Unknown character '\\'"
    );
    let (buffer, _) = draw(&app, 80, 24);
    let row = row_text(&buffer, 2);
    assert!(
        row.trim_end()
            .ends_with("◀ Syntax error at line 20: Unknown character '\\'"),
        "{row:?}"
    );
    assert!(!row_text(&buffer, 1).contains('◀'));
    assert!(!row_text(&buffer, 3).contains('◀'));
    let x = row_text(&buffer, 2).chars().position(|c| c == '◀').unwrap() as u16;
    assert_eq!(buffer[(x, 2)].fg, Color::Red);
}

#[test]
fn a_parse_error_gets_the_text_that_run_gives() {
    let app = app_with_lines(&["10 X = 1 2"]);
    let marks = app.marks();
    assert_eq!(marks.len(), 1, "{marks:?}");
    assert_eq!(
        marks[0].message,
        "Syntax error at line 10: Unexpected 2 after expression"
    );
}

#[test]
fn the_mark_goes_when_the_fault_is_corrected() {
    let mut app = app_with_lines(&["10 PRINT 2 \\"]);
    assert_eq!(app.marks().len(), 1);
    app.key(press(KeyCode::Backspace));
    assert_eq!(app.marks().len(), 0, "{:?}", app.marks());
}

#[test]
fn a_line_with_no_number_and_a_second_line_with_a_number_get_a_mark() {
    let app = app_with_lines(&["10 PRINT 1", "PRINT 2", "10 PRINT 3"]);
    let marks: Vec<(usize, String)> = app
        .marks()
        .iter()
        .map(|m| (m.index, m.message.clone()))
        .collect();
    assert_eq!(
        marks,
        [
            (1, "No line number".to_string()),
            (2, "Line number 10 used twice".to_string())
        ]
    );
}

#[test]
fn tab_changes_the_mode_and_puts_no_character_in_the_text() {
    let mut app = app_with_lines(&["10 PRINT 1"]);
    app.key(press(KeyCode::Left));
    app.key(press(KeyCode::Tab));
    assert_eq!(app.mode(), Mode::Run);
    app.key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
    assert_eq!(app.mode(), Mode::Edit);
    assert_eq!(lines(&app), ["10 PRINT 1"]);
    assert_eq!(app.cursor(), (0, 9));
}

#[test]
fn the_stored_program_after_the_mode_change_equals_the_typed_text() {
    let typed = ["10 FOR I% = 1 TO 3", "20 PRINT I%", "30 NEXT", "40 END"];
    let mut app = App::new(None);
    type_lines(&mut app, typed[0], &["PRINT I%", "NEXT", "END"]);
    assert!(app.program().is_empty());
    app.key(press(KeyCode::Tab));
    let listed: Vec<String> = app
        .program()
        .list()
        .iter()
        .map(|(_, line)| detokenize(line).unwrap())
        .collect();
    assert_eq!(listed, typed);
}

#[test]
fn the_stored_program_is_sorted_and_leaves_out_the_marked_lines() {
    let mut app = app_with_lines(&["30 END", "10 PRINT 1", "20 PRINT \\"]);
    app.key(press(KeyCode::Tab));
    assert_eq!(app.program().get_line_numbers(), [10, 30]);
    // The text stays as typed.
    assert_eq!(lines(&app), ["30 END", "10 PRINT 1", "20 PRINT \\"]);
}

#[test]
fn the_editor_keeps_the_spaces_of_the_user() {
    let mut app = app_with_lines(&["10   PRINT  1"]);
    app.key(press(KeyCode::Tab));
    app.key(press(KeyCode::Tab));
    app.key(press(KeyCode::Tab));
    assert_eq!(app.mode(), Mode::Edit);
    assert_eq!(lines(&app), ["10   PRINT  1"]);
    assert_eq!(app.program().len(), 1);
}

#[test]
fn the_changed_mark_shows_after_the_first_change() {
    let mut app = App::new(Some("demo.bbas".to_string()));
    let status = row_text(&draw(&app, 80, 24).0, 23);
    assert!(status.contains("demo.bbas "), "{status:?}");
    assert!(!status.contains('*'), "{status:?}");
    app.key(press(KeyCode::Right));
    assert!(!app.document().is_changed());
    type_text(&mut app, "1");
    assert!(app.document().is_changed());
    let status = row_text(&draw(&app, 80, 24).0, 23);
    assert!(status.contains("demo.bbas *"), "{status:?}");
}

#[test]
fn the_status_line_shows_the_line_the_column_the_lines_and_the_bytes() {
    let mut app = App::new(None);
    type_lines(&mut app, "10 PRINT 1", &["END"]);
    app.key(press(KeyCode::Left));
    // The program has 2 lines: "10 PRINT 1" uses 4 + 2 bytes and "20 END" uses
    // 4 + 1 bytes, then 2 bytes end the program.
    let status = row_text(&draw(&app, 100, 24).0, 23);
    assert!(
        status.contains("Ln 2 Col 6   2 lines   13 bytes"),
        "{status:?}"
    );
}

#[test]
fn ctrl_n_renumbers_the_program_and_writes_the_listed_form() {
    let mut app = app_with_lines(&["5 GOTO 7", "7 PRINT  \"X\""]);
    app.key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
    assert_eq!(lines(&app), ["10 GOTO 20", "20 PRINT \"X\""]);
    assert!(app.document().is_changed());
}

#[test]
fn ctrl_n_with_a_marked_line_does_nothing_and_says_why() {
    let mut app = app_with_lines(&["5 GOTO 7", "7 PRINT \\"]);
    app.key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
    assert_eq!(lines(&app), ["5 GOTO 7", "7 PRINT \\"]);
    assert_eq!(app.message(), "Correct the marked lines first");
}

#[test]
fn ctrl_n_says_which_line_has_a_jump_to_a_line_that_does_not_exist() {
    let mut app = app_with_lines(&["5 GOTO 99", "7 END"]);
    app.key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
    assert_eq!(lines(&app), ["10 GOTO 99", "20 END"]);
    assert_eq!(app.message(), "Failed at 10");
}

#[test]
fn a_tab_character_becomes_2_spaces() {
    let mut app = app_with_lines(&["10 REM"]);
    app.key(press(KeyCode::Char('\t')));
    assert_eq!(lines(&app), ["10 REM  "]);
}

#[test]
fn a_paste_puts_the_text_with_its_line_numbers_and_tabs_as_2_spaces() {
    let mut app = App::new(None);
    app.paste("10 PRINT 1\r\n20\tEND\n");
    assert_eq!(lines(&app), ["10 PRINT 1", "20  END", ""]);
    assert_eq!(app.cursor(), (2, 0));
    assert!(app.document().is_changed());
}

#[test]
fn a_paste_in_another_mode_does_not_change_the_text() {
    let mut app = App::new(None);
    app.key(press(KeyCode::Tab));
    app.paste("10 END");
    assert_eq!(lines(&app), [""]);
}

/// A path in the temporary directory that is unique for the test.
fn temp_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("issue_170_{}_{}", std::process::id(), name))
}

#[test]
fn ide_with_a_file_loads_the_file_text() {
    let path = temp_path("load.bbas");
    std::fs::write(&path, "10 PRINT  \"HI\"\n20 END\n").unwrap();
    let app = App::new(Some(path.to_string_lossy().into_owned()));
    std::fs::remove_file(&path).unwrap();
    assert_eq!(lines(&app), ["10 PRINT  \"HI\"", "20 END"]);
    assert!(!app.document().is_changed());
    assert_eq!(app.message(), "");
    assert_eq!(app.marks().len(), 0);
}

#[test]
fn ide_with_a_file_that_does_not_exist_starts_an_empty_document_with_that_name() {
    let path = temp_path("missing.bbas");
    let name = path.to_string_lossy().into_owned();
    let app = App::new(Some(name.clone()));
    assert_eq!(lines(&app), [""]);
    assert_eq!(app.document().file_name(), Some(name.as_str()));
    assert_eq!(app.message(), "");
}

#[test]
fn ide_with_a_file_that_cannot_be_read_gives_a_message() {
    let path = temp_path("a_directory");
    std::fs::create_dir_all(&path).unwrap();
    let name = path.to_string_lossy().into_owned();
    let app = App::new(Some(name.clone()));
    std::fs::remove_dir(&path).unwrap();
    assert_eq!(lines(&app), [""]);
    assert!(
        app.message().starts_with(&format!("Cannot read {}", name)),
        "{}",
        app.message()
    );
}
