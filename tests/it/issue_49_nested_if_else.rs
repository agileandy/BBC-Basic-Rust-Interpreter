//! Issue 49: an ELSE on a line with more than one IF.
//!
//! The BBC BASIC rule: a false IF continues after the next ELSE on the line,
//! at any depth. If there is no ELSE after it, the run continues with the
//! next line. When the run gets to an ELSE, it skips the rest of the line.

use crate::common::run_timed_stops as run;

/// Run line 20 after `A%` and `B%` get each value, and compare the output
/// with the expected output for each pair (A%, B%).
fn check_each_value(line_20: &str, expected: [((i32, i32), &str); 4]) {
    for ((a, b), output_expected) in expected {
        let line_10 = format!("10 A%={} : B%={}", a, b);
        let (result, output) = run(&[&line_10, line_20, "30 PRINT \"end\""]);

        assert_eq!(result, Ok(()), "A%={} B%={}", a, b);
        assert_eq!(output, output_expected, "A%={} B%={}", a, b);
    }
}

#[test]
fn test_if_then_statement_then_inner_if_else() {
    check_each_value(
        "20 IF A% THEN PRINT \"x\" : IF B% THEN PRINT \"y\" ELSE PRINT \"z\"",
        [
            ((0, 0), "z\nend\n"),
            ((0, 1), "z\nend\n"),
            ((1, 0), "x\nz\nend\n"),
            ((1, 1), "x\ny\nend\n"),
        ],
    );
}

#[test]
fn test_if_then_inner_if_else() {
    check_each_value(
        "20 IF A% THEN IF B% THEN PRINT \"y\" ELSE PRINT \"z\"",
        [
            ((0, 0), "z\nend\n"),
            ((0, 1), "z\nend\n"),
            ((1, 0), "z\nend\n"),
            ((1, 1), "y\nend\n"),
        ],
    );
}

#[test]
fn test_if_else_inner_if_else() {
    check_each_value(
        "20 IF A% THEN PRINT \"x\" ELSE IF B% THEN PRINT \"y\" ELSE PRINT \"z\"",
        [
            ((0, 0), "z\nend\n"),
            ((0, 1), "y\nend\n"),
            ((1, 0), "x\nend\n"),
            ((1, 1), "x\nend\n"),
        ],
    );
}

#[test]
fn test_if_then_inner_if_with_no_else_continues_with_next_line() {
    check_each_value(
        "20 IF A% THEN IF B% THEN PRINT \"y\"",
        [
            ((0, 0), "end\n"),
            ((0, 1), "end\n"),
            ((1, 0), "end\n"),
            ((1, 1), "y\nend\n"),
        ],
    );
}

#[test]
fn test_if_else_with_inner_if_with_no_else() {
    check_each_value(
        "20 IF A% THEN PRINT \"x\" ELSE IF B% THEN PRINT \"y\"",
        [
            ((0, 0), "end\n"),
            ((0, 1), "y\nend\n"),
            ((1, 0), "x\nend\n"),
            ((1, 1), "x\nend\n"),
        ],
    );
}

#[test]
fn test_gosub_and_proc_in_each_branch_of_if_else_if_else() {
    let line_20 =
        "20 IF A% THEN GOSUB 100 : PROCx ELSE IF B% THEN GOSUB 200 : PROCy ELSE GOSUB 300 : PROCz";
    let expected = [
        ((0, 0), "g3\npz\nend\n"),
        ((0, 1), "g2\npy\nend\n"),
        ((1, 0), "g1\npx\nend\n"),
        ((1, 1), "g1\npx\nend\n"),
    ];
    for ((a, b), output_expected) in expected {
        let line_10 = format!("10 A%={} : B%={}", a, b);
        let (result, output) = run(&[
            &line_10,
            line_20,
            "30 PRINT \"end\"",
            "40 END",
            "100 PRINT \"g1\" : RETURN",
            "200 PRINT \"g2\" : RETURN",
            "300 PRINT \"g3\" : RETURN",
            "400 DEF PROCx : PRINT \"px\" : ENDPROC",
            "500 DEF PROCy : PRINT \"py\" : ENDPROC",
            "600 DEF PROCz : PRINT \"pz\" : ENDPROC",
        ]);

        assert_eq!(result, Ok(()), "A%={} B%={}", a, b);
        assert_eq!(output, output_expected, "A%={} B%={}", a, b);
    }
}

#[test]
fn test_nested_if_else_in_for_loop_on_one_line() {
    // I%=1: outer true, inner false, so the ELSE part and NEXT run.
    // I%=2: outer false, so the run continues after the ELSE.
    // I%=3: both true, so the ELSE skips the rest of the line, with NEXT.
    let (result, output) = run(&[
        "10 FOR I%=1 TO 3 : IF I%<>2 THEN IF I%=3 THEN PRINT \"b\" ELSE PRINT \"a\" : NEXT",
        "20 PRINT \"end\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "a\na\nb\nend\n");
}

#[test]
fn test_else_in_rem_after_inner_then_is_part_of_the_rem() {
    // The REM uses the rest of the line, so the line has no ELSE.
    check_each_value(
        "20 IF A% THEN IF B% THEN REM note ELSE PRINT \"z\"",
        [
            ((0, 0), "end\n"),
            ((0, 1), "end\n"),
            ((1, 0), "end\n"),
            ((1, 1), "end\n"),
        ],
    );
}
