//! Issue 48: a line number after THEN or ELSE is a short form of GOTO.

use crate::common::run_timed_stops as run;

#[test]
fn test_if_true_then_line_number_jumps() {
    let (result, output) = run(&[
        "10 X = 1",
        "20 IF X THEN 40",
        "30 PRINT \"SKIPPED\"",
        "40 PRINT \"TARGET\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "TARGET\n");
}

#[test]
fn test_if_false_then_line_number_continues_with_next_line() {
    let (result, output) = run(&[
        "10 X = 0",
        "20 IF X THEN 40",
        "30 PRINT \"NEXT\"",
        "40 PRINT \"TARGET\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "NEXT\nTARGET\n");
}

#[test]
fn test_if_then_line_number_else_line_number() {
    let program = |x: &'static str| {
        run(&[
            x,
            "20 IF X THEN 50 ELSE 70",
            "30 PRINT \"SKIPPED\"",
            "40 END",
            "50 PRINT \"THEN\"",
            "60 END",
            "70 PRINT \"ELSE\"",
        ])
    };

    assert_eq!(program("10 X = 1"), (Ok(()), "THEN\n".to_string()));
    assert_eq!(program("10 X = 0"), (Ok(()), "ELSE\n".to_string()));
}

#[test]
fn test_if_then_print_else_line_number() {
    let program = |x: &'static str| {
        run(&[
            x,
            "20 IF X THEN PRINT \"A\" ELSE 50",
            "30 PRINT \"NEXT\"",
            "40 END",
            "50 PRINT \"ELSE\"",
        ])
    };

    assert_eq!(program("10 X = 1"), (Ok(()), "A\nNEXT\n".to_string()));
    assert_eq!(program("10 X = 0"), (Ok(()), "ELSE\n".to_string()));
}

#[test]
fn test_if_then_line_number_else_print() {
    let program = |x: &'static str| {
        run(&[
            x,
            "20 IF X THEN 50 ELSE PRINT \"B\"",
            "30 PRINT \"NEXT\"",
            "40 END",
            "50 PRINT \"THEN\"",
        ])
    };

    assert_eq!(program("10 X = 1"), (Ok(()), "THEN\n".to_string()));
    assert_eq!(program("10 X = 0"), (Ok(()), "B\nNEXT\n".to_string()));
}

#[test]
fn test_if_then_missing_line_gives_goto_error() {
    let (result, _) = run(&["10 IF 1 THEN 99"]);

    assert_eq!(result, Err("No such line: 99 at line 10".to_string()));
}

#[test]
fn test_if_then_line_number_jumps_out_of_for_body_each_time() {
    let (result, output) = run(&[
        "10 FOR I% = 1 TO 3",
        "20 IF I% > 0 THEN 40",
        "30 PRINT \"SKIPPED\"",
        "40 PRINT I%",
        "50 NEXT I%",
        "60 PRINT \"DONE\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         1\n         2\n         3\nDONE\n");
}

#[test]
fn test_if_then_line_number_jumps_out_of_inner_for_loop_3_times() {
    let (result, output) = run(&[
        "10 FOR I% = 1 TO 3",
        "20 FOR J% = 1 TO 10",
        "30 IF J% = 2 THEN 60",
        "40 NEXT J%",
        "60 PRINT I% * 10 + J%",
        "70 NEXT I%",
        "80 PRINT \"DONE\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "        12\n        22\n        32\nDONE\n");
}
