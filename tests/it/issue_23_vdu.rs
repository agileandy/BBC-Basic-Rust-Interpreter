//! Issue #23: VDU sends bytes to the display. A control code waits for its
//! parameter bytes, then has the same effect as its statement.

use crate::common::run_lines as run;
use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;

#[test]
fn test_vdu_prints_printable_characters() {
    let (result, output) = run(&["10 VDU 65, 66"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "AB");
}

#[test]
fn test_vdu_uses_the_low_8_bits_of_a_value() {
    let (result, output) = run(&["10 VDU 256 + 67"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "C");
}

#[test]
fn test_vdu_127_prints_nothing() {
    let (result, output) = run(&["10 VDU 127"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "");
}

#[test]
fn test_vdu_22_selects_a_mode() {
    // Mode 7 has no graphics. VDU 22, 1 selects mode 1, so the plot is in the buffer.
    let (result, output) = run(&[
        "10 MODE 7",
        "20 VDU 22, 1",
        "30 PLOT 69, 10, 10",
        "40 PRINT POINT(10, 10)",
    ]);

    assert_eq!(result, Ok(()));
    assert!(output.ends_with("3\n"), "output: {:?}", output);
}

#[test]
fn test_vdu_25_moves_then_draws_a_line() {
    let (result, output) = run(&[
        "10 VDU 25, 4, 100; 200;",
        "20 VDU 25, 5, 300; 200;",
        "30 PRINT POINT(200, 200); POINT(50, 100)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "10\n");
}

#[test]
fn test_plot_4_moves_and_plot_5_draws_a_line() {
    // VDU 25 uses PLOT, so PLOT 4 (absolute move) and PLOT 5 (absolute draw) must work
    let (result, output) = run(&[
        "10 PLOT 4, 100, 200",
        "20 PLOT 5, 300, 200",
        "30 PRINT POINT(200, 200); POINT(50, 100)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "10\n");
}

#[test]
fn test_plot_0_and_plot_1_are_relative() {
    let (result, output) = run(&[
        "10 MOVE 100, 100",
        "20 PLOT 0, 0, 100",
        "30 PLOT 1, 200, 0",
        "40 PRINT POINT(200, 200); POINT(150, 150)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "10\n");
}

#[test]
fn test_vdu_25_with_negative_coordinates() {
    // -10 as 2 bytes is 246, 255. ORIGIN moves 0, 0 so that -10, -10 is on the canvas.
    let (result, output) = run(&[
        "10 ORIGIN 100, 100",
        "20 VDU 25, 69, -10; -10;",
        "30 PRINT POINT(-10, -10)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n");
}

#[test]
fn test_vdu_29_sets_the_origin() {
    let (result, output) = run(&[
        "10 VDU 29, 100; 50;",
        "20 PLOT 69, 0, 0",
        "30 ORIGIN 0, 0",
        "40 PRINT POINT(100, 50)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n");
}

#[test]
fn test_vdu_16_clears_the_graphics() {
    let (result, output) = run(&["10 PLOT 69, 10, 10", "20 VDU 16", "30 PRINT POINT(10, 10)"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0\n");
}

#[test]
fn test_vdu_18_sets_the_graphics_colour() {
    let (result, output) = run(&[
        "10 VDU 18, 0, 0",
        "20 PLOT 69, 10, 10",
        "30 PRINT POINT(10, 10)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0\n");
}

#[test]
fn test_vdu_12_sets_the_print_column_to_0() {
    let (result, output) = run(&["10 PRINT \"AB\";", "20 VDU 12", "30 PRINT TAB(3); \"X\""]);

    assert_eq!(result, Ok(()));
    assert!(output.ends_with("AB   X\n"), "output: {:?}", output);
}

#[test]
fn test_vdu_13_sets_the_print_column_to_0() {
    let (result, output) = run(&["10 PRINT \"AB\";", "20 VDU 13", "30 PRINT TAB(3); \"X\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "AB\r   X\n");
}

#[test]
fn test_vdu_30_sets_the_print_column_to_0() {
    let (result, output) = run(&["10 PRINT \"AB\";", "20 VDU 30", "30 PRINT TAB(3); \"X\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "AB   X\n");
}

#[test]
fn test_vdu_8_and_9_change_the_print_column() {
    // After AB the column is 2. VDU 8 makes it 1, VDU 9, 9 makes it 3.
    let (result, output) = run(&[
        "10 PRINT \"AB\";",
        "20 VDU 8, 9, 9",
        "30 PRINT TAB(5); \"X\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "AB\x08  X\n");
}

#[test]
fn test_vdu_31_sets_the_print_column() {
    let (result, output) = run(&["10 VDU 31, 4, 10", "20 PRINT TAB(6); \"X\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "  X\n");
}

#[test]
fn test_vdu_10_starts_a_new_line() {
    let (result, output) = run(&["10 VDU 65, 10, 66"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "A\nB");
}

#[test]
fn test_vdu_7_prints_nothing() {
    let (result, output) = run(&["10 VDU 7"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "");
}

#[test]
fn test_vdu_command_split_over_2_statements() {
    let (result, output) = run(&[
        "10 VDU 25, 4, 100;",
        "20 VDU 200;",
        "30 VDU 25",
        "40 VDU 5, 300; 200;",
        "50 PRINT POINT(200, 200)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n");
}

#[test]
fn test_vdu_23_takes_9_parameter_bytes_and_prints_nothing() {
    let (result, output) = run(&[
        "10 VDU 23, 240, 65, 66, 67, 68, 69, 70, 71, 72",
        "20 VDU 65",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "A");
}

#[test]
fn test_vdu_19_takes_5_parameter_bytes_and_prints_nothing() {
    let (result, output) = run(&["10 VDU 19, 1, 2, 0, 0, 0", "20 VDU 65"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "A");
}

#[test]
fn test_vdu_17_takes_1_parameter_byte() {
    let (result, output) = run(&["10 VDU 17, 65, 66"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "B");
}

#[test]
fn test_run_empties_the_byte_queue() {
    // Line 10 starts a VDU 23 command and does not complete it.
    let mut executor = Executor::new();
    let mut program = ProgramStore::new();
    program.store_line(tokenize("10 VDU 23, 1").expect("line tokenizes"));
    assert_eq!(run_program(&mut executor, &mut program), Ok(()));

    let mut program = ProgramStore::new();
    program.store_line(tokenize("10 VDU 65").expect("line tokenizes"));
    assert_eq!(run_program(&mut executor, &mut program), Ok(()));

    assert_eq!(executor.get_output(), "A");
}

#[test]
fn test_clear_empties_the_byte_queue() {
    let (result, output) = run(&["10 VDU 23, 1", "20 CLEAR", "30 VDU 65"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "A");
}
