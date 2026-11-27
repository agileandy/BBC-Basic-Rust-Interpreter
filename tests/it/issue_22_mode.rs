//! Issue #22: MODE selects a screen mode, clears the screen and resets the graphics.

use crate::common::run_lines as run;

#[test]
fn test_each_mode_from_0_to_7_runs_with_no_error() {
    for mode in 0..=7 {
        let line = format!("10 MODE {}", mode);
        let (result, _) = run(&[&line, "20 PRINT \"OK\""]);

        assert_eq!(result, Ok(()), "MODE {} gives an error", mode);
    }
}

#[test]
fn test_mode_256_gives_error() {
    let (result, output) = run(&["10 MODE 256", "20 PRINT \"NOT HERE\""]);

    let message = result.expect_err("MODE 256 must give an error");
    assert!(message.contains("at line 10"), "message: {}", message);
    assert_eq!(output, "");
}

#[test]
fn test_negative_mode_gives_error() {
    let (result, _) = run(&["10 MODE -1"]);

    assert!(result.is_err());
}

#[test]
fn test_mode_error_number_is_25() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 MODE 256",
        "30 END",
        "100 PRINT ERR",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "25\n");
}

#[test]
fn test_mode_clears_the_graphics_buffer() {
    let (result, output) = run(&[
        "10 MODE 1",
        "20 PLOT 69, 100, 100",
        "30 PRINT POINT(100, 100)",
        "40 MODE 1",
        "50 PRINT POINT(100, 100)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n0\n");
}

#[test]
fn test_mode_sets_the_print_column_to_0() {
    let (result, output) = run(&["10 PRINT \"AB\";", "20 MODE 4", "30 PRINT TAB(3); \"X\""]);

    assert_eq!(result, Ok(()));
    assert!(output.ends_with("AB   X\n"), "output: {:?}", output);
}

#[test]
fn test_mode_moves_the_graphics_cursor_to_0_0() {
    let (result, output) = run(&[
        "10 MODE 1",
        "20 MOVE 500, 500",
        "30 MODE 1",
        "40 DRAW 0, 0",
        "50 PRINT POINT(250, 250)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0\n");
}

#[test]
fn test_mode_sets_the_origin_to_0_0() {
    let (result, output) = run(&[
        "10 MODE 1",
        "20 ORIGIN 100, 0",
        "30 MODE 1",
        "40 PLOT 69, 0, 0",
        "50 ORIGIN 0, 0",
        "60 PRINT POINT(0, 0)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n");
}

#[test]
fn test_mode_sets_the_default_colours() {
    let (result, output) = run(&[
        "10 MODE 1",
        "20 GCOL 0, 0",
        "30 MODE 1",
        "40 PLOT 69, 10, 10",
        "50 PRINT POINT(10, 10)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n");
}

#[test]
fn test_graphics_do_nothing_in_a_text_only_mode() {
    for mode in [3, 6, 7] {
        let mode_line = format!("10 MODE {}", mode);
        let (result, output) = run(&[
            &mode_line,
            "20 PLOT 69, 10, 10",
            "30 MOVE 0, 100",
            "40 DRAW 200, 100",
            "50 PRINT POINT(10, 10); POINT(100, 100)",
        ]);

        assert_eq!(result, Ok(()), "MODE {}", mode);
        assert_eq!(output, "00\n", "MODE {}", mode);
    }
}

#[test]
fn test_graphics_draw_in_a_graphics_mode() {
    for mode in [0, 1, 2, 4, 5] {
        let mode_line = format!("10 MODE {}", mode);
        let (result, output) = run(&[
            &mode_line,
            "20 PLOT 69, 10, 10",
            "30 MOVE 0, 100",
            "40 DRAW 200, 100",
            "50 PRINT POINT(10, 10); POINT(100, 100)",
        ]);

        assert_eq!(result, Ok(()), "MODE {}", mode);
        assert_eq!(output, "11\n", "MODE {}", mode);
    }
}
