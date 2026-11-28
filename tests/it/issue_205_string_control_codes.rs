//! Issue #205: in the IDE, a control character in a printed string acts as
//! the same VDU code. The executor column follows the column of the screen,
//! so POS and TAB(x) agree with the screen after a wrap. The prompt mode
//! does not change.

use crate::common::run_lines;
use bbc_basic_interpreter::ui::App;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::Terminal;
use std::time::Duration;

/// The time limit for a run that must end.
const LIMIT: Duration = Duration::from_secs(5);
/// A terminal of 120 by 40 has a pane of 120 by 38. The text screen of
/// mode 7 (40 by 25) is in the centre: columns 40 to 79, rows 1 to 25.
const BIG: (u16, u16) = (120, 40);
const MODE_7_LEFT: u16 = 40;

fn draw(app: &App) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(BIG.0, BIG.1)).expect("test terminal");
    terminal.draw(|frame| app.draw(frame)).expect("draw");
    terminal.backend().buffer().clone()
}

/// Run the program with Ctrl-R, and give the rows of the mode 7 text
/// screen, with no spaces at the ends.
fn run(text: &str) -> Vec<String> {
    let mut app = App::new(None);
    app.paste(text);
    draw(&app);
    app.key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    assert!(app.wait_for_run(LIMIT), "the run did not end");
    let buffer = draw(&app);
    (1..=25)
        .map(|y| {
            (MODE_7_LEFT..MODE_7_LEFT + 40)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

#[test]
fn chr_10_in_a_string_keeps_the_column_as_vdu_10() {
    let string = run("10 MODE 7\n\
         20 PRINT \"AB\";CHR$(10);\"C\";\n\
         30 P% = POS\n\
         40 PRINT TAB(0, 10);P%;");
    let vdu = run("10 MODE 7\n\
         20 PRINT \"AB\";:VDU 10:PRINT \"C\";\n\
         30 P% = POS\n\
         40 PRINT TAB(0, 10);P%;");
    assert_eq!(string[..2], ["AB", "  C"], "{string:?}");
    assert_eq!(string[10], "3");
    assert_eq!(string, vdu);
}

#[test]
fn chr_8_at_column_0_goes_to_the_row_above_as_vdu_8() {
    let string = run("10 MODE 7\n\
         20 PRINT TAB(0, 3);CHR$(8);\"C\";\n\
         30 P% = POS\n\
         40 PRINT TAB(0, 10);P%;");
    let vdu = run("10 MODE 7\n\
         20 PRINT TAB(0, 3);:VDU 8:PRINT \"C\";\n\
         30 P% = POS\n\
         40 PRINT TAB(0, 10);P%;");
    // C is at column 39 of row 2, then the cursor wraps to column 0
    assert_eq!(string[2], format!("{}C", " ".repeat(39)), "{string:?}");
    assert_eq!(string[3], "");
    assert_eq!(string[10], "0");
    assert_eq!(string, vdu);
}

#[test]
fn chr_8_in_a_line_goes_back_one_column_as_vdu_8() {
    let string = run("10 MODE 7\n\
         20 PRINT \"AB\";CHR$(8);\"C\";\n\
         30 P% = POS\n\
         40 PRINT TAB(0, 10);P%;");
    let vdu = run("10 MODE 7\n\
         20 PRINT \"AB\";:VDU 8:PRINT \"C\";\n\
         30 P% = POS\n\
         40 PRINT TAB(0, 10);P%;");
    assert_eq!(string[0], "AC", "{string:?}");
    assert_eq!(string[10], "2");
    assert_eq!(string, vdu);
}

#[test]
fn chr_13_in_a_string_goes_to_column_0_as_vdu_13() {
    let string = run("10 MODE 7\n\
         20 PRINT \"ABC\";CHR$(13);\"X\";\n\
         30 P% = POS\n\
         40 PRINT TAB(0, 10);P%;");
    let vdu = run("10 MODE 7\n\
         20 PRINT \"ABC\";:VDU 13:PRINT \"X\";\n\
         30 P% = POS\n\
         40 PRINT TAB(0, 10);P%;");
    assert_eq!(string[0], "XBC", "{string:?}");
    assert_eq!(string[10], "1");
    assert_eq!(string, vdu);
}

#[test]
fn chr_31_takes_its_2_parameters_from_the_string_as_vdu_31() {
    // "#" is code 35: a parameter, not a character to show
    let string = run("10 MODE 7\n\
         20 PRINT CHR$(31);\"#\";CHR$(2);\"X\";\n\
         30 P% = POS\n\
         40 PRINT TAB(0, 10);P%;");
    let vdu = run("10 MODE 7\n\
         20 VDU 31, 35, 2\n\
         30 PRINT \"X\";\n\
         40 P% = POS\n\
         50 PRINT TAB(0, 10);P%;");
    assert_eq!(string[2], format!("{}X", " ".repeat(35)), "{string:?}");
    assert_eq!(string[10], "36");
    assert_eq!(string, vdu);
}

#[test]
fn tab_after_a_wrap_uses_the_column_of_the_screen() {
    let rows = run("10 MODE 7\n\
         20 PRINT STRING$(45, \"x\");TAB(10)\"y\";\n\
         30 P% = POS\n\
         40 PRINT TAB(0, 10);P%;");
    assert_eq!(rows[0], "x".repeat(40), "{rows:?}");
    assert_eq!(rows[1], "xxxxx     y");
    assert_eq!(rows[2], "");
    assert_eq!(rows[10], "11");
}

#[test]
fn the_new_line_at_the_end_of_print_goes_to_column_0() {
    let rows = run("10 MODE 7\n\
         20 PRINT \"AB\"\n\
         30 PRINT \"C\";");
    assert_eq!(rows[..2], ["AB", "C"], "{rows:?}");
}

#[test]
fn the_prompt_mode_output_does_not_change() {
    let (result, output) = run_lines(&[
        "10 PRINT \"AB\";CHR$(10);\"C\";CHR$(8);\"D\";CHR$(13);\"E\"",
        "20 PRINT ;POS",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "AB\nC\x08D\rE\n0\n");
}
