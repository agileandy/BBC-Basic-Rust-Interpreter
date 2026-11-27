//! Issue #159: an apostrophe in PRINT and INPUT prints a new line and sets
//! the column to 0. An apostrophe at the start of a line or a statement is
//! still a comment.

use crate::common::{run_lines as run, run_with_limit, run_with_replies};
use bbc_basic_interpreter::tokenizer::{detokenize, tokenize};

#[test]
fn test_apostrophe_between_two_strings_prints_two_lines() {
    let (result, output) = run(&["10 PRINT \"A\"'\"B\""]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "A\nB\n");
}

#[test]
fn test_two_apostrophes_print_two_new_lines_first() {
    let (result, output) = run(&["10 PRINT ''\"X\""]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "\n\nX\n");
}

#[test]
fn test_apostrophe_at_the_end_adds_a_new_line() {
    let (result, output) = run(&["10 PRINT \"A\"'"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "A\n\n");
}

#[test]
fn test_apostrophe_before_a_variable() {
    let (result, output) = run(&["10 A = 7", "20 PRINT 'A"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "\n         7\n");
}

#[test]
fn test_apostrophe_sets_the_column_to_zero_for_tab_and_pos() {
    let (result, output) = run(&["10 PRINT TAB(5);\"A\"'TAB(5);\"B\";", "20 PRINT POS"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "     A\n     B         6\n");
}

#[test]
fn test_apostrophe_after_a_semicolon_and_a_comma() {
    let (result, output) = run(&["10 PRINT \"A\";'\"B\",'\"C\""]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "A\nB         \nC\n");
}

#[test]
fn test_apostrophe_after_then_and_a_colon_in_print() {
    let (result, output) = run(&[
        "10 IF 1 THEN PRINT \"A\"'\"B\"",
        "20 X = 1 : PRINT \"C\"'\"D\"",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "A\nB\nC\nD\n");
}

#[test]
fn test_apostrophe_in_a_string_is_text() {
    let (result, output) = run(&["10 PRINT \"it's\""]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "it's\n");
}

#[test]
fn test_apostrophe_in_a_rem_and_in_data_is_text() {
    let (result, output) = run(&[
        "10 REM it's a comment",
        "20 READ A$",
        "30 PRINT A$",
        "40 DATA it's",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "it's\n");
}

#[test]
fn test_input_prompt_with_apostrophe_prints_two_lines() {
    let (result, output) = run_with_replies(&["10 INPUT \"A\"'\"B\" X", "20 PRINT X"], &["42"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "A\nB\n        42\n");
}

#[test]
fn test_input_line_prompt_with_apostrophe() {
    let (result, output) = run_with_replies(
        &["10 INPUT LINE \"Name\"'\"> \" N$", "20 PRINT N$"],
        &["Ann"],
    );
    assert_eq!(result, Ok(()));
    assert_eq!(output, "Name\n> \nAnn\n");
}

#[test]
fn test_apostrophe_in_print_file_is_a_syntax_error() {
    let (result, _) = run(&["10 PRINT#1, \"A\"'\"B\""]);
    assert_eq!(
        result,
        Err("Syntax error at line 10: ' is not permitted in PRINT#".to_string())
    );
}

#[test]
fn test_line_that_starts_with_apostrophe_is_a_comment() {
    let line = tokenize("10 ' a comment").expect("line tokenizes");
    assert_eq!(line.line_number, Some(10));
    let (result, output) = run_with_limit(&["10 ' a comment", "20 PRINT \"OK\""]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "OK\n");
}

#[test]
fn test_statement_that_starts_with_apostrophe_is_a_comment() {
    let (result, output) = run(&["10 PRINT \"OK\" : ' a comment", "20 PRINT \"END\""]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "OK\nEND\n");
}

#[test]
fn test_list_shows_the_apostrophe_as_typed() {
    for source in [
        "10 PRINT \"A\"'\"B\"",
        "10 PRINT ''\"X\"",
        "10 PRINT \"A\"'",
        "10 PRINT TAB(5);\"A\"'TAB(5);\"B\"",
        "10 INPUT \"A\"'\"B\" X",
    ] {
        let line = tokenize(source).expect("line tokenizes");
        assert_eq!(detokenize(&line).expect("line lists"), source);
    }
}
