//! Issue 78: INPUT and INPUT LINE show a prompt text, read the reply and store it.
//!
//! BBC BASIC rule: a prompt followed by a comma or a semicolon shows the text and
//! then `?`. A prompt followed directly by the variable shows only the text. A
//! variable with no prompt before it shows `?`.

use crate::common::run_with_replies;
use bbc_basic_interpreter::tokenizer::{detokenize, tokenize};

/// Run `INPUT <items>` with one reply, then print A$ in brackets.
fn prompt_output(input_line: &str, reply: &str) -> String {
    let (result, output) = run_with_replies(&[input_line, "20 PRINT \"[\";A$;\"]\""], &[reply]);
    assert_eq!(result, Ok(()));
    output
}

#[test]
fn test_prompt_with_semicolon_shows_question_mark() {
    assert_eq!(
        prompt_output("10 INPUT \"Name\";A$", "Bob"),
        "Name?\n[Bob]\n"
    );
}

#[test]
fn test_prompt_with_comma_shows_question_mark() {
    assert_eq!(
        prompt_output("10 INPUT \"Name\",A$", "Bob"),
        "Name?\n[Bob]\n"
    );
}

#[test]
fn test_prompt_with_no_separator_shows_no_question_mark() {
    assert_eq!(
        prompt_output("10 INPUT \"Name: \" A$", "Bob"),
        "Name: \n[Bob]\n"
    );
}

#[test]
fn test_variable_with_no_prompt_shows_question_mark() {
    assert_eq!(prompt_output("10 INPUT A$", "Bob"), "?\n[Bob]\n");
}

#[test]
fn test_prompts_can_repeat() {
    let (result, output) = run_with_replies(
        &["10 INPUT \"X\", X, \"Y\" Y", "20 PRINT X", "30 PRINT Y"],
        &["1", "2"],
    );
    assert_eq!(result, Ok(()));
    assert_eq!(output, "X?\nY\n         1\n         2\n");
}

#[test]
fn test_two_variables_with_two_replies() {
    let (result, output) =
        run_with_replies(&["10 INPUT A, B", "20 PRINT A", "30 PRINT B"], &["3", "4"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "?\n?\n         3\n         4\n");
}

#[test]
fn test_two_variables_with_one_reply_that_has_a_comma() {
    let (result, output) =
        run_with_replies(&["10 INPUT A, B", "20 PRINT A", "30 PRINT B"], &["3,4"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "?\n         3\n         4\n");
}

#[test]
fn test_extra_values_on_a_reply_line_are_ignored() {
    let (result, output) = run_with_replies(
        &["10 INPUT A", "20 INPUT B", "30 PRINT A", "40 PRINT B"],
        &["3,9", "4"],
    );
    assert_eq!(result, Ok(()));
    assert_eq!(output, "?\n?\n         3\n         4\n");
}

#[test]
fn test_string_and_numeric_variable_in_one_statement() {
    let (result, output) = run_with_replies(
        &[
            "10 INPUT \"Name and age\", N$, A%",
            "20 PRINT N$",
            "30 PRINT A%",
        ],
        &["Bob, 7"],
    );
    assert_eq!(result, Ok(()));
    assert_eq!(output, "Name and age?\nBob\n         7\n");
}

#[test]
fn test_input_line_with_prompt() {
    assert_eq!(
        prompt_output("10 INPUT LINE \"Text\" A$", " a, b "),
        "Text\n[ a, b ]\n"
    );
}

#[test]
fn test_input_line_with_prompt_and_comma() {
    assert_eq!(
        prompt_output("10 INPUT LINE \"Text\", A$", "a, b"),
        "Text?\n[a, b]\n"
    );
}

/// LIST removes the spaces after a comma in each statement, so the source here
/// has no such spaces.
#[test]
fn test_list_shows_input_prompt_as_typed() {
    for source in [
        "10 INPUT \"Name\";A$",
        "10 INPUT \"X\",X,\"Y\" Y",
        "10 INPUT LINE \"Text\" A$",
    ] {
        let line = tokenize(source).expect("line tokenizes");
        assert_eq!(detokenize(&line).expect("line detokenizes"), source);
    }
}
