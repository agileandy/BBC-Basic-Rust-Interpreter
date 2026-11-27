//! Issue #173: the run mode of the IDE shows the graphics canvas. Each
//! character cell shows 2 samples with the upper half block: the foreground
//! is the top sample and the background is the bottom sample. The picture
//! keeps the aspect 4 to 3 in the centre of the pane, and text that is not
//! a space shows above it.
//!
//! The tests draw with the test backend of ratatui. A terminal of 100 by 34
//! gives a pane of 100 by 32 cells and a picture area of 85 by 32 cells at
//! column 7: 85 by 64 samples.

use bbc_basic_interpreter::ui::App;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::Color;
use ratatui::Terminal;
use std::time::{Duration, Instant};

/// The time limit for a run that must end.
const LIMIT: Duration = Duration::from_secs(5);

/// The upper half block: the top sample is the foreground.
const HALF: &str = "\u{2580}";

/// The 8 steady colours of a BBC Micro, in colour number order (the
/// palette of issue #174 in `src/ui/theme.rs`).
const RED: Color = Color::Red;
const YELLOW: Color = Color::Yellow;
const BLUE: Color = Color::Blue;
const WHITE: Color = Color::White;
const STEADY: [Color; 8] = [
    Color::Black,
    RED,
    Color::Green,
    YELLOW,
    BLUE,
    Color::Magenta,
    Color::Cyan,
    WHITE,
];

/// The picture area of a terminal of 100 by 34: the terminal width and
/// height, the column of the area and its width in cells. The first pane
/// row is terminal row 1.
const BIG: (u16, u16, u16, u16) = (100, 34, 7, 85);

/// The first column of the text screen of mode 1 (40 columns) in the pane
/// of 100 columns: the screen is in the centre (issue #174).
const MODE_1_TEXT: usize = 30;

fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(letter: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(letter), KeyModifiers::CONTROL)
}

fn draw(app: &App, width: u16, height: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
    terminal.draw(|frame| app.draw(frame)).expect("draw");
    terminal.backend().buffer().clone()
}

/// Go to the run mode, draw the pane at the size, then run the program and
/// wait for its end.
fn run_at(text: &str, width: u16, height: u16) -> App {
    let mut app = App::new(None);
    app.paste(text);
    app.key(press(KeyCode::Tab));
    draw(&app, width, height);
    app.key(ctrl('r'));
    assert!(app.wait_for_run(LIMIT), "the run did not end");
    app
}

fn run_big(text: &str) -> (App, Buffer) {
    let app = run_at(text, BIG.0, BIG.1);
    let buffer = draw(&app, BIG.0, BIG.1);
    (app, buffer)
}

/// The physical colour number (0 to 7) of a colour of the picture.
fn number(colour: Color) -> u8 {
    STEADY
        .iter()
        .position(|&steady| steady == colour)
        .unwrap_or_else(|| panic!("{colour:?} is not a colour of the palette")) as u8
}

/// The 2 samples of the cell at picture column `x` and picture row `y`, or
/// None if the cell shows text.
fn cell(buffer: &Buffer, x: u16, y: u16) -> Option<(u8, u8)> {
    let cell = &buffer[(BIG.2 + x, 1 + y)];
    match cell.symbol() {
        HALF => Some((number(cell.fg), number(cell.bg))),
        " " => Some((0, 0)),
        _ => None,
    }
}

/// The samples of the picture area: `columns` by 2 times `rows`.
fn samples(buffer: &Buffer, left: u16, columns: u16, rows: u16) -> Vec<Vec<u8>> {
    let mut grid = vec![vec![0; usize::from(columns)]; 2 * usize::from(rows)];
    for y in 0..rows {
        for x in 0..columns {
            let cell = &buffer[(left + x, 1 + y)];
            if cell.symbol() == HALF {
                grid[2 * usize::from(y)][usize::from(x)] = number(cell.fg);
                grid[2 * usize::from(y) + 1][usize::from(x)] = number(cell.bg);
            }
        }
    }
    grid
}

/// The count of cells of the pane that show the upper half block.
fn picture_cells(buffer: &Buffer) -> usize {
    buffer
        .content()
        .iter()
        .filter(|cell| cell.symbol() == HALF)
        .count()
}

/// The terminal row and column where the text starts, if a pane row has it.
fn find_text(buffer: &Buffer, text: &str) -> Option<(u16, usize)> {
    (1..buffer.area.height - 1).find_map(|row| {
        let symbols: Vec<&str> = (0..buffer.area.width)
            .map(|x| buffer[(x, row)].symbol())
            .collect();
        let row_text = symbols.concat();
        let byte = row_text.find(text)?;
        // The column is the count of symbols before the byte offset
        let column = symbols
            .iter()
            .scan(0, |end, symbol| {
                *end += symbol.len();
                Some(*end)
            })
            .take_while(|&end| end <= byte)
            .count();
        Some((row, column))
    })
}

#[test]
fn a_diagonal_goes_from_the_bottom_left_to_the_top_right_of_the_picture() {
    let (_app, buffer) = run_big("10 MODE 1 : MOVE 0,0 : DRAW 1279,1023");
    let grid = samples(&buffer, BIG.2, BIG.3, 32);
    assert_eq!(grid[63][0], 7, "the bottom left sample");
    assert_eq!(grid[0][84], 7, "the top right sample");
    assert_eq!(grid[0][0], 0, "the top left sample");
    assert_eq!(grid[63][84], 0, "the bottom right sample");
    // Each sample row has the line, and the line goes to the right as it
    // goes up
    let columns: Vec<usize> = grid
        .iter()
        .map(|row| row.iter().position(|&s| s != 0).expect("a set sample"))
        .collect();
    assert!(
        columns.windows(2).all(|pair| pair[0] >= pair[1]),
        "{columns:?}"
    );
    // No picture outside the picture area
    assert_eq!(buffer[(BIG.2 - 1, 32)].symbol(), " ");
    assert_eq!(buffer[(BIG.2 + BIG.3, 1)].symbol(), " ");
}

#[test]
fn a_filled_rectangle_fills_its_cells() {
    // The rectangle covers x 320 to 960 and y 256 to 768: picture rows 8 to
    // 23 and picture columns 22 to 62 are in it in full
    let (_app, buffer) = run_big("10 MODE 1 : RECTANGLE FILL 320, 256, 640, 512");
    for y in 8..=23 {
        for x in 22..=62 {
            assert_eq!(cell(&buffer, x, y), Some((7, 7)), "cell {x}, {y}");
        }
    }
    assert_eq!(cell(&buffer, 10, 8), Some((0, 0)));
    assert_eq!(cell(&buffer, 40, 2), Some((0, 0)));
}

#[test]
fn a_cell_shows_2_colours_the_top_sample_and_the_bottom_sample() {
    // Red above y 687 and yellow from y 0 to 687: canvas row 336 is the
    // first yellow row, and it is the first row of sample 21 (16 rows each)
    let (_app, buffer) = run_big(
        "10 MODE 1 : GCOL 0, 1 : RECTANGLE FILL 0, 0, 1279, 1023\n\
         20 GCOL 0, 2 : RECTANGLE FILL 0, 0, 1279, 687",
    );
    let both = &buffer[(BIG.2 + 40, 1 + 10)];
    assert_eq!(both.symbol(), HALF);
    assert_eq!((both.fg, both.bg), (RED, YELLOW));
    assert_eq!(cell(&buffer, 40, 9), Some((1, 1)));
    assert_eq!(cell(&buffer, 40, 11), Some((3, 3)));
}

#[test]
fn mode_0_draws_colour_1_in_white() {
    let (_app, buffer) = run_big("10 MODE 0 : GCOL 0, 1 : RECTANGLE FILL 0, 0, 1279, 1023");
    assert_eq!(buffer[(BIG.2 + 40, 10)].fg, WHITE);
    assert_eq!(buffer[(BIG.2 + 40, 10)].bg, WHITE);
}

/// Draw a vertical stripe of 80 units for each colour, from x 0. Stripe n
/// is at picture columns from about n * 5.3 to (n + 1) * 5.3.
fn stripes(mode: u8, colours: &[u8]) -> Buffer {
    let mut text = format!("10 MODE {mode}\n");
    for (n, colour) in colours.iter().enumerate() {
        text.push_str(&format!(
            "{} GCOL 0, {colour} : RECTANGLE FILL {}, 0, 79, 1023\n",
            20 + 10 * n,
            80 * n
        ));
    }
    run_big(&text).1
}

/// The foreground colour of the middle of stripe n.
fn stripe(buffer: &Buffer, n: u16) -> Color {
    let x = BIG.2 + (n * 80 + 40) * BIG.3 / 1280;
    assert_eq!(buffer[(x, 16)].symbol(), HALF, "stripe {n}");
    buffer[(x, 16)].fg
}

#[test]
fn mode_1_draws_its_4_colours_black_red_yellow_white() {
    let buffer = stripes(1, &[0, 1, 2, 3]);
    assert_eq!(buffer[(BIG.2 + 2, 16)].symbol(), " ", "colour 0");
    assert_eq!(stripe(&buffer, 1), RED);
    assert_eq!(stripe(&buffer, 2), YELLOW);
    assert_eq!(stripe(&buffer, 3), WHITE);
}

#[test]
fn mode_2_draws_8_colours_and_a_flash_colour_as_its_first_steady_colour() {
    let buffer = stripes(2, &[0, 1, 2, 3, 4, 5, 6, 7, 9, 12]);
    for (n, colour) in STEADY.iter().enumerate().skip(1) {
        assert_eq!(stripe(&buffer, n as u16), *colour, "colour {n}");
    }
    // 9 flashes red and cyan, 12 flashes blue and yellow
    assert_eq!(stripe(&buffer, 8), RED);
    assert_eq!(stripe(&buffer, 9), BLUE);
}

#[test]
fn clg_clears_the_picture_and_keeps_the_text() {
    let (_app, buffer) =
        run_big("10 MODE 1 : RECTANGLE FILL 0, 0, 1279, 1023 : PRINT \"KEEP\" : CLG");
    assert_eq!(picture_cells(&buffer), 0);
    assert_eq!(find_text(&buffer, "KEEP").map(|at| at.1), Some(MODE_1_TEXT));
}

#[test]
fn cls_clears_the_text_and_keeps_the_picture() {
    let (_app, buffer) =
        run_big("10 MODE 1 : RECTANGLE FILL 0, 0, 1279, 1023 : PRINT \"GONE\" : CLS");
    assert_eq!(find_text(&buffer, "GONE"), None);
    assert_eq!(cell(&buffer, 40, 10), Some((7, 7)));
}

#[test]
fn text_printed_after_a_drawing_shows_above_it() {
    let (_app, buffer) =
        run_big("10 MODE 1 : RECTANGLE FILL 0, 0, 1279, 1023 : PRINT TAB(20);\"HELLO\"");
    let (row, column) = find_text(&buffer, "HELLO").expect("the text");
    assert_eq!(column, MODE_1_TEXT + 20);
    assert_eq!(buffer[(column as u16, row)].fg, WHITE);
    // The spaces before the text and the cells after it show the picture
    assert_eq!(buffer[(column as u16 - 1, row)].symbol(), HALF);
    assert_eq!(buffer[(column as u16 + 5, row)].symbol(), HALF);
    // The picture covers the border of the text screen
    assert_eq!(buffer[(MODE_1_TEXT as u16 - 1, row)].symbol(), HALF);
}

#[test]
fn a_mode_with_no_graphics_shows_no_picture() {
    let (_app, buffer) = run_big(
        "10 MODE 1 : RECTANGLE FILL 0, 0, 1279, 1023\n20 MODE 3 : RECTANGLE FILL 0, 0, 1279, 1023",
    );
    assert_eq!(picture_cells(&buffer), 0);
}

#[test]
fn a_program_that_draws_nothing_shows_no_picture() {
    let (_app, buffer) = run_big("10 PRINT \"TEXT\"");
    assert_eq!(picture_cells(&buffer), 0);
    assert!(find_text(&buffer, "TEXT").is_some());
}

#[test]
fn a_pane_resize_scales_the_picture_again() {
    // A terminal of 60 by 20 has a pane of 60 by 18 and a picture of 48 by
    // 18 cells. The run draws at that size, then the pane becomes large. A
    // scale of the small picture gives a thick line: only a new sample of
    // the canvas gives the picture of a run at the large size.
    let program = "10 MODE 1 : MOVE 0,0 : DRAW 1279,1023";
    let (_big, buffer) = run_big(program);
    let expected = samples(&buffer, BIG.2, BIG.3, 32);

    let mut app = run_at(program, 60, 20);
    let first = samples(&draw(&app, 60, 20), 6, 48, 18);
    assert_eq!(first[35][0], 7);
    assert_eq!(first[0][47], 7);
    // The first draw at the new size has only the small picture
    let scaled = samples(&draw(&app, BIG.0, BIG.1), BIG.2, BIG.3, 32);
    assert_eq!(scaled[63][0], 7);
    assert_ne!(scaled, expected);
    let deadline = Instant::now() + LIMIT;
    loop {
        if samples(&draw(&app, BIG.0, BIG.1), BIG.2, BIG.3, 32) == expected {
            break;
        }
        assert!(Instant::now() < deadline, "the picture did not scale");
        app.wait_for_run(Duration::from_millis(10));
    }
}
