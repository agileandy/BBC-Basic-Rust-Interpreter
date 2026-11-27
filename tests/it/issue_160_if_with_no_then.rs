//! Issue 160: IF can have a statement directly after its condition, with no
//! THEN. The condition ends before the first keyword that starts a statement.

use crate::common::{run_binary_read_first, run_timed_stops as run};

#[test]
fn test_if_with_no_then_runs_print() {
    let (result, output) = run(&["10 IF 1 = 1 PRINT \"YES\"", "20 IF 1 = 2 PRINT \"NO\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "YES\n");
}

#[test]
fn test_if_with_no_then_goto_jumps_only_when_true() {
    let program = |a: &'static str| {
        run(&[
            a,
            "20 IF A GOTO 100",
            "30 PRINT \"NEXT\"",
            "40 END",
            "100 PRINT \"TARGET\"",
        ])
    };

    assert_eq!(program("10 A = 1"), (Ok(()), "TARGET\n".to_string()));
    assert_eq!(program("10 A = 0"), (Ok(()), "NEXT\n".to_string()));
}

#[test]
fn test_then_line_number_and_then_goto_still_jump() {
    let program = |line: &'static str| {
        run(&[
            "10 A = 1",
            line,
            "30 PRINT \"NEXT\"",
            "40 END",
            "100 PRINT \"TARGET\"",
        ])
    };

    assert_eq!(
        program("20 IF A THEN 100"),
        (Ok(()), "TARGET\n".to_string())
    );
    assert_eq!(
        program("20 IF A THEN GOTO 100"),
        (Ok(()), "TARGET\n".to_string())
    );
}

#[test]
fn test_assignment_with_no_then_is_a_syntax_error() {
    let (result, _) = run(&["10 A% = 5", "20 IF A% > 3 A% = 0"]);

    assert_eq!(
        result,
        Err("Syntax error at line 20: Expected THEN after IF condition".to_string())
    );
}

#[test]
fn test_assignment_with_no_then_before_a_colon_is_a_syntax_error() {
    let (result, _) = run(&["10 A% = 5", "20 IF A% > 3 A% = 0 : PRINT A%"]);

    assert_eq!(
        result,
        Err("Syntax error at line 20: Expected THEN after IF condition".to_string())
    );
}

#[test]
fn test_if_with_no_then_else_runs_the_correct_branch() {
    let program = |a: &'static str| run(&[a, "20 IF A PRINT 1 ELSE PRINT 2"]);

    assert_eq!(program("10 A = 1"), (Ok(()), "1\n".to_string()));
    assert_eq!(program("10 A = 0"), (Ok(()), "2\n".to_string()));
}

#[test]
fn test_user_function_condition_then_proc_call() {
    let (result, output) = run(&[
        "10 IF FNt(1) PROCa",
        "20 IF FNt(0) PROCa",
        "30 END",
        "40 DEF FNt(X) = X",
        "50 DEF PROCa",
        "60 PRINT \"A\"",
        "70 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "A\n");
}

#[test]
fn test_function_keyword_in_condition_is_not_a_statement_start() {
    let (result, output) = run(&[
        "10 A$ = \"abc\"",
        "20 IF LEN A$ > 2 PRINT \"long\"",
        "30 IF SIN 0 = 0 PRINT \"zero\"",
        "40 IF TIME >= 0 PRINT \"time\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "long\nzero\ntime\n");
}

#[test]
fn test_nested_if_with_no_then() {
    let program = |a: &'static str, b: &'static str| run(&[a, b, "30 IF A IF B PRINT 1"]);

    assert_eq!(program("10 A = 1", "20 B = 1"), (Ok(()), "1\n".to_string()));
    assert_eq!(program("10 A = 1", "20 B = 0"), (Ok(()), String::new()));
    assert_eq!(program("10 A = 0", "20 B = 1"), (Ok(()), String::new()));
}

#[test]
fn test_if_with_no_then_runs_all_statements_of_the_line() {
    let (result, output) = run(&["10 IF 1 PRINT \"A\": PRINT \"B\"", "20 PRINT \"C\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "A\nB\nC\n");
}

#[test]
fn test_condition_alone_with_no_then_is_a_syntax_error() {
    let (result, _) = run(&["10 IF 1 = 1"]);

    assert_eq!(
        result,
        Err("Syntax error at line 10: Expected THEN after IF condition".to_string())
    );
}

#[test]
fn test_list_shows_the_line_with_no_then() {
    let output = run_binary_read_first("10 IF 1 = 1 PRINT \"YES\"\nLIST\nRUN\nEXIT\n");

    assert!(
        output.contains("10 IF 1 = 1 PRINT \"YES\"\n"),
        "LIST output: {output}"
    );
    assert!(!output.contains("THEN"), "LIST output: {output}");
    assert!(output.contains("YES\n"), "RUN output: {output}");
}
