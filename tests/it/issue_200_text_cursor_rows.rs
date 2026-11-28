//! Issue #200: in the IDE, PRINT TAB(x, y) and VDU 31 move the text cursor
//! to a row and a column of the text screen, and the cursor codes of VDU
//! move it by one place. VPOS gives the row. The prompt mode does not
//! change.

use crate::common::run_lines;
use bbc_basic_interpreter::executor::{Console, Executor, Interrupt};
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;
use bbc_basic_interpreter::ui::App;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::Terminal;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

/// The time limit for a run that must end.
const LIMIT: Duration = Duration::from_secs(5);
/// A terminal of 120 by 40 has a pane of 120 by 38. The text screen of
/// mode 0 (80 by 32) is in the centre: columns 20 to 99, rows 1 to 32.
const BIG: (u16, u16) = (120, 40);
const MODE_0_LEFT: u16 = 20;

fn draw(app: &App, width: u16, height: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
    terminal.draw(|frame| app.draw(frame)).expect("draw");
    terminal.backend().buffer().clone()
}

/// Run the program with Ctrl-R, and draw the frame at 120 by 40.
fn run(text: &str) -> Buffer {
    let mut app = App::new(None);
    app.paste(text);
    draw(&app, BIG.0, BIG.1);
    app.key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    assert!(app.wait_for_run(LIMIT), "the run did not end");
    draw(&app, BIG.0, BIG.1)
}

/// The rows of the mode 0 text screen, with no spaces at the ends.
fn mode_0_rows(buffer: &Buffer) -> Vec<String> {
    (1..=32)
        .map(|y| {
            (MODE_0_LEFT..MODE_0_LEFT + 80)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

/// The character at column `x` and row `y` of the mode 0 text screen.
fn at(buffer: &Buffer, x: u16, y: u16) -> String {
    buffer[(MODE_0_LEFT + x, 1 + y)].symbol().to_string()
}

#[test]
fn print_tab_x_y_puts_the_text_at_the_column_and_the_row() {
    let buffer = run("10 MODE 0\n20 PRINT TAB(5, 3); \"X\";");
    let rows = mode_0_rows(&buffer);
    assert_eq!(at(&buffer, 5, 3), "X", "{rows:?}");
    // The prompt starts on a new row below the cursor
    assert_eq!(rows[..5], ["", "", "", "     X", ">"]);
}

#[test]
fn a_menu_of_4_lines_shows_at_its_rows() {
    let buffer = run("10 MODE 0\n\
         20 FOR I% = 0 TO 3\n\
         30 PRINT TAB(10, 2 + 2 * I%); \"ITEM\";I%\n\
         40 NEXT\n\
         50 PRINT TAB(0, 12);");
    let rows = mode_0_rows(&buffer);
    let item = |n: usize| format!("{}ITEM{}", " ".repeat(10), n);
    assert_eq!(rows[2], item(0), "{rows:?}");
    assert_eq!(rows[4], item(1));
    assert_eq!(rows[6], item(2));
    assert_eq!(rows[8], item(3));
    assert_eq!([&rows[3], &rows[5], &rows[7]], ["", "", ""]);
}

#[test]
fn vdu_31_0_0_then_a_print_writes_over_the_first_row() {
    let buffer = run("10 MODE 0\n\
         20 PRINT \"AAAA\"\n\
         30 PRINT \"BBBB\"\n\
         40 VDU 31, 0, 0\n\
         50 PRINT \"C\";");
    let rows = mode_0_rows(&buffer);
    assert_eq!(rows[0], "CAAA", "{rows:?}");
    // The prompt starts on the row below the cursor, over the old text
    assert_eq!(rows[1], "> BB");
}

#[test]
fn the_cursor_codes_move_the_cursor_and_vpos_and_pos_give_it() {
    let buffer = run("10 MODE 0\n\
         20 DIM P%(7), V%(7)\n\
         30 PRINT TAB(5, 5);\n\
         40 P%(0) = POS : V%(0) = VPOS\n\
         50 VDU 8 : P%(1) = POS : V%(1) = VPOS\n\
         60 VDU 9, 9 : P%(2) = POS : V%(2) = VPOS\n\
         70 VDU 10 : P%(3) = POS : V%(3) = VPOS\n\
         80 VDU 11, 11 : P%(4) = POS : V%(4) = VPOS\n\
         90 VDU 30 : P%(5) = POS : V%(5) = VPOS\n\
         100 VDU 31, 7, 2 : P%(6) = POS : V%(6) = VPOS\n\
         110 CLS : P%(7) = POS : V%(7) = VPOS\n\
         120 FOR I% = 0 TO 7 : PRINT ;P%(I%);\",\";V%(I%) : NEXT");
    let rows = mode_0_rows(&buffer);
    assert_eq!(
        rows[..9],
        ["5,5", "4,5", "6,5", "6,6", "6,4", "0,0", "7,2", "0,0", ">"]
    );
}

#[test]
fn a_cursor_code_prints_nothing() {
    let buffer = run("10 MODE 0\n\
         20 PRINT \"ABCD\";\n\
         30 VDU 8, 8, 10, 11, 9\n\
         40 PRINT \"X\";");
    let rows = mode_0_rows(&buffer);
    assert_eq!(rows[0], "ABCX", "{rows:?}");
}

#[test]
fn vdu_13_goes_to_the_start_of_the_row() {
    let buffer = run("10 MODE 0\n20 PRINT TAB(6, 2); \"AB\";\n30 VDU 13\n40 PRINT \"X\";");
    let rows = mode_0_rows(&buffer);
    assert_eq!(rows[2], "X     AB", "{rows:?}");
}

#[test]
fn a_print_at_the_last_column_continues_on_the_next_row() {
    let buffer = run("10 MODE 0\n20 PRINT TAB(78, 3); \"ABCD\";");
    let rows = mode_0_rows(&buffer);
    assert_eq!(at(&buffer, 78, 3) + &at(&buffer, 79, 3), "AB", "{rows:?}");
    assert_eq!(rows[4], "CD");
}

#[test]
fn a_line_feed_at_the_last_row_scrolls_the_screen_up_one_row() {
    let buffer = run("10 MODE 0\n\
         20 PRINT TAB(0, 30); \"TOP\"\n\
         30 PRINT TAB(0, 31); \"BOTTOM\"\n\
         40 V% = VPOS\n\
         50 VDU 31, 0, 0\n\
         60 PRINT ;V%;");
    let rows = mode_0_rows(&buffer);
    assert_eq!(rows[29], "TOP", "{rows:?}");
    assert_eq!(rows[30], "BOTTOM");
    assert_eq!(rows[0], "31");
}

#[test]
fn a_move_outside_the_text_screen_does_nothing() {
    let buffer =
        run("10 MODE 0\n20 PRINT TAB(3, 1); \"A\";\n30 VDU 31, 80, 0, 31, 0, 32\n40 PRINT \"B\";");
    let rows = mode_0_rows(&buffer);
    assert_eq!(rows[1], "   AB", "{rows:?}");
}

#[test]
fn the_picture_of_mode_0_takes_the_text_screen() {
    // The text screen of mode 0 shows in full and has near the aspect 4 to
    // 3: the picture covers the text screen, so a cell of text and the
    // graphics at the same place are together
    let buffer = run("10 MODE 0 : RECTANGLE FILL 0, 0, 1279, 1023");
    let picture: Vec<(u16, u16)> = (0..BIG.0)
        .flat_map(|x| (0..BIG.1).map(move |y| (x, y)))
        .filter(|&at| buffer[at].symbol() == "\u{2580}")
        .collect();
    let left = picture.iter().map(|at| at.0).min();
    let right = picture.iter().map(|at| at.0).max();
    let top = picture.iter().map(|at| at.1).min();
    let bottom = picture.iter().map(|at| at.1).max();
    assert_eq!(
        (left, right, top, bottom),
        (Some(20), Some(99), Some(1), Some(32))
    );
}

#[test]
fn in_the_prompt_mode_vpos_and_the_row_stay_as_before() {
    let (result, output) = run_lines(&["10 PRINT TAB(5, 3); \"X\";", "20 PRINT ;VPOS;\" \";POS"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "X0 8\n");
}

#[test]
fn before_the_first_mode_the_text_screen_is_the_pane() {
    // A terminal of 80 by 24: the pane has 22 rows from row 1, and no MODE
    let mut app = App::new(None);
    app.paste("10 CLS\n20 PRINT TAB(4, 20); \"Z\";\n30 V% = VPOS\n40 PRINT TAB(0, 2);V%");
    draw(&app, 80, 24);
    app.key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    assert!(app.wait_for_run(LIMIT), "the run did not end");
    let buffer = draw(&app, 80, 24);
    assert_eq!(buffer[(4, 21)].symbol(), "Z");
    assert_eq!(buffer[(0, 3)].symbol(), "2");
    assert_eq!(buffer[(1, 3)].symbol(), "0");
}

// The corrections of the code review

/// The rows of the mode 7 text screen (40 by 25) at 120 by 40: columns 40
/// to 79, rows 1 to 25.
fn mode_7_rows(buffer: &Buffer) -> Vec<String> {
    (1..=25)
        .map(|y| {
            (40..80)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

#[test]
fn a_print_in_the_last_column_wraps_at_once() {
    // A BBC Micro gives VPOS 1 and POS 0 after 40 characters in mode 7
    let buffer = run("10 MODE 7\n\
         20 PRINT STRING$(40, \"x\");\n\
         30 V% = VPOS : P% = POS\n\
         40 VDU 11\n\
         50 PRINT \"Z\";\n\
         60 PRINT TAB(0, 3);V%;\",\";P%");
    let rows = mode_7_rows(&buffer);
    // VDU 11 goes up from column 0 of row 1 to column 0 of row 0
    assert_eq!(rows[0], format!("Z{}", "x".repeat(39)), "{rows:?}");
    assert_eq!(rows[3], "1,0");
}

#[test]
fn a_print_in_the_last_column_of_the_last_row_scrolls_at_once() {
    let buffer = run("10 MODE 7\n\
         20 PRINT TAB(0, 24);STRING$(40, \"y\");\n\
         30 V% = VPOS\n\
         40 PRINT TAB(0, 0);V%");
    let rows = mode_7_rows(&buffer);
    assert_eq!(rows[23], "y".repeat(40), "{rows:?}");
    assert_eq!(rows[0], "24");
}

#[test]
fn vdu_11_at_the_top_row_scrolls_the_screen_down() {
    let buffer =
        run("10 MODE 0\n20 PRINT \"A\";\n30 VDU 30, 11\n40 PRINT \"B\";\n50 PRINT TAB(0, 5);");
    let rows = mode_0_rows(&buffer);
    assert_eq!(rows[..2], ["B", "A"], "{rows:?}");
}

#[test]
fn vdu_8_at_the_top_left_scrolls_down_and_goes_to_the_last_column() {
    let buffer = run("10 MODE 0\n\
         20 PRINT \"A\";\n\
         30 VDU 30, 8\n\
         40 V% = VPOS : P% = POS\n\
         50 PRINT TAB(0, 5);V%;\",\";P%");
    let rows = mode_0_rows(&buffer);
    assert_eq!(rows[1], "A", "{rows:?}");
    assert_eq!(rows[5], "0,79");
}

#[test]
fn the_prompt_does_not_write_over_text_after_a_carriage_return() {
    let buffer = run("10 MODE 0\n20 PRINT \"RESULT\";CHR$(13);");
    let rows = mode_0_rows(&buffer);
    assert_eq!(rows[..2], ["RESULT", ">"], "{rows:?}");
}

#[test]
fn the_prompt_does_not_write_over_text_after_a_move_to_the_top() {
    let buffer = run("10 MODE 0\n20 PRINT \"ABC\" : VDU 31, 0, 0");
    let rows = mode_0_rows(&buffer);
    assert_eq!(rows[..2], ["ABC", ">"], "{rows:?}");
}

/// A console with no cursor moves: it keeps the text that it gets.
struct TextOnly(Rc<RefCell<String>>);

impl Console for TextOnly {
    fn write(&mut self, text: &str) {
        self.0.borrow_mut().push_str(text);
    }
    fn clear(&mut self) {}
    fn read_line(&mut self) -> Option<String> {
        None
    }
    fn read_key(&mut self, _limit: Option<Duration>) -> Option<char> {
        None
    }
    fn interrupt(&mut self) -> Option<Interrupt> {
        None
    }
}

#[test]
fn a_console_with_no_cursor_moves_gets_vdu_8_and_vdu_10_as_text() {
    let text = Rc::new(RefCell::new(String::new()));
    let mut executor = Executor::new();
    executor.set_console(Box::new(TextOnly(Rc::clone(&text))));
    let mut program = ProgramStore::new();
    program.store_line(tokenize("10 VDU 65, 8, 66, 10, 67").expect("line tokenizes"));
    assert_eq!(run_program(&mut executor, &mut program), Ok(()));
    assert_eq!(*text.borrow(), "A\x08B\nC");
}
