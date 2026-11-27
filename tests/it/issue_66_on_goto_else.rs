//! Issue 66: ON GOTO and ON GOSUB with an ELSE part.
//!
//! The BBC BASIC rule: an ON with an index out of range continues after the
//! next ELSE on the line, in the same way as a false IF. If no ELSE follows
//! the ON on the line, the error "ON range" (ERR 40) occurs. An ON with an
//! index in range jumps, and the ELSE part does not run.

use crate::common::run_timed_stops as run;

/// Run line 20 after `X%` gets each value, with lines 100 and 200 that print
/// "one" and "two", and compare the output with the expected output.
fn check_each_value(line_20: &str, line_100: &str, line_200: &str, expected: &[(i32, &str)]) {
    for (x, output_expected) in expected {
        let line_10 = format!("10 X%={}", x);
        let (result, output) = run(&[
            &line_10,
            line_20,
            "30 PRINT \"end\"",
            "40 END",
            line_100,
            line_200,
        ]);

        assert_eq!(result, Ok(()), "X%={}", x);
        assert_eq!(output, *output_expected, "X%={}", x);
    }
}

const GOTO_100: &str = "100 PRINT \"one\" : END";
const GOTO_200: &str = "200 PRINT \"two\" : END";
const GOSUB_100: &str = "100 PRINT \"one\" : RETURN";
const GOSUB_200: &str = "200 PRINT \"two\" : RETURN";

#[test]
fn test_on_goto_else_in_range_jumps() {
    check_each_value(
        "20 ON X% GOTO 100,200 ELSE PRINT \"bad\"",
        GOTO_100,
        GOTO_200,
        &[(1, "one\n"), (2, "two\n")],
    );
}

#[test]
fn test_on_goto_else_out_of_range_runs_else_part() {
    check_each_value(
        "20 ON X% GOTO 100,200 ELSE PRINT \"bad\"",
        GOTO_100,
        GOTO_200,
        &[(0, "bad\nend\n"), (3, "bad\nend\n"), (-1, "bad\nend\n")],
    );
}

#[test]
fn test_on_goto_else_part_is_rest_of_line() {
    check_each_value(
        "20 ON X% GOTO 100,200 ELSE PRINT \"bad\" : PRINT \"more\"",
        GOTO_100,
        GOTO_200,
        &[(2, "two\n"), (5, "bad\nmore\nend\n")],
    );
}

#[test]
fn test_on_gosub_else_in_range_returns_after_else_part() {
    check_each_value(
        "20 ON X% GOSUB 100,200 ELSE PRINT \"bad\" : PRINT \"more\"",
        GOSUB_100,
        GOSUB_200,
        &[(1, "one\nend\n"), (2, "two\nend\n")],
    );
}

#[test]
fn test_on_gosub_else_out_of_range_runs_else_part() {
    check_each_value(
        "20 ON X% GOSUB 100,200 ELSE PRINT \"bad\"",
        GOSUB_100,
        GOSUB_200,
        &[(0, "bad\nend\n"), (5, "bad\nend\n")],
    );
}

#[test]
fn test_on_else_part_with_inner_on_else() {
    check_each_value(
        "20 ON X% GOTO 100 ELSE ON X% GOTO 300,200 ELSE PRINT \"none\"",
        GOTO_100,
        GOTO_200,
        &[(1, "one\n"), (2, "two\n"), (5, "none\nend\n")],
    );
}

#[test]
fn test_on_goto_out_of_range_with_no_else_gives_on_range_error() {
    let (result, output) = run(&["10 X%=5", "20 ON X% GOTO 100,200", "30 PRINT \"end\""]);

    let message = result.expect_err("ON range error");
    assert!(message.contains("line 20"), "message: {}", message);
    assert_eq!(output, "");
}

#[test]
fn test_on_gosub_out_of_range_with_no_else_gives_on_range_error() {
    let (result, output) = run(&["10 X%=0", "20 ON X% GOSUB 100,200", "30 PRINT \"end\""]);

    let message = result.expect_err("ON range error");
    assert!(message.contains("line 20"), "message: {}", message);
    assert_eq!(output, "");
}

#[test]
fn test_on_range_error_is_err_40() {
    for keyword in ["GOTO", "GOSUB"] {
        let line_20 = format!("20 ON X% {} 100,200", keyword);
        let (result, output) = run(&[
            "5 ON ERROR GOTO 300",
            "10 X%=5",
            &line_20,
            "30 PRINT \"end\"",
            "40 END",
            "100 END",
            "200 END",
            "300 PRINT ERR : END",
        ]);

        assert_eq!(result, Ok(()), "{}", keyword);
        assert_eq!(output, "        40\n", "{}", keyword);
    }
}

/// The program of the issue facts: lines 100 and 200 do not exist, and the
/// IF is false for X% = 1 and 2.
#[test]
fn test_issue_facts_program_prints_bad_then_end() {
    for x in [1, 2, 5] {
        let line_10 = format!("10 X%={}", x);
        let (result, output) = run(&[
            &line_10,
            "20 IF X%=5 THEN ON X% GOTO 100,200 ELSE PRINT \"bad\"",
            "30 PRINT \"end\"",
        ]);

        assert_eq!(result, Ok(()), "X%={}", x);
        assert_eq!(output, "bad\nend\n", "X%={}", x);
    }
}

#[test]
fn test_if_then_on_goto_else_belongs_to_on() {
    check_each_value(
        "20 IF X%>0 THEN ON X% GOTO 100,200 ELSE PRINT \"bad\"",
        GOTO_100,
        GOTO_200,
        &[
            (1, "one\n"),
            (2, "two\n"),
            (5, "bad\nend\n"),
            (0, "bad\nend\n"),
        ],
    );
}

#[test]
fn test_if_then_on_gosub_else_belongs_to_on() {
    check_each_value(
        "20 IF X%>0 THEN ON X% GOSUB 100,200 ELSE PRINT \"bad\"",
        GOSUB_100,
        GOSUB_200,
        &[(1, "one\nend\n"), (2, "two\nend\n"), (5, "bad\nend\n")],
    );
}

#[test]
fn test_on_error_goto_keeps_its_behaviour() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 X%=1/0",
        "30 PRINT \"no\"",
        "100 PRINT \"handled\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "handled\n");
}
