//! Issue 167: keyword short forms, such as P. for PRINT and L. for LIST.

use crate::common::{run_binary_read_first, run_lines};
use bbc_basic_interpreter::tokenizer::{detokenize, tokenize};

#[test]
fn short_form_of_print_prints_a_string_and_a_real() {
    let (result, output) = run_lines(&["10 P.\"A\";1.5"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "A1.5\n");
}

#[test]
fn short_form_before_a_number_with_a_full_stop_runs() {
    let (result, output) = run_lines(&["10 P. .5"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "0.5\n");
}

#[test]
fn short_forms_in_a_loop_run() {
    let (result, output) = run_lines(&["10 F.I=1 TO 3:P.I;:N.", "20 REP.:I=I-1:U.I=1"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "123");
}

#[test]
fn short_form_of_list_at_the_prompt_lists_the_program() {
    let output = run_binary_read_first("10 P.\"HI\"\nL.\nEXIT\n");
    assert!(output.contains("10 PRINT \"HI\"\n"), "output: {output}");
}

#[test]
fn listed_line_shows_the_full_keyword_and_runs() {
    let listed = detokenize(&tokenize("10 P.\"HI\":G.30").unwrap()).unwrap();
    assert_eq!(listed, "10 PRINT \"HI\":GOTO 30");
    let (result, output) = run_lines(&[&listed, "20 PRINT \"NO\"", "30 END"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "HI\n");
}

#[test]
fn name_that_is_not_a_short_form_and_a_full_stop_gives_the_error_of_today() {
    let output = run_binary_read_first("X=1:PRINT X.\nEXIT\n");
    assert!(
        output.contains("Error: Syntax error: Unknown character '.'\n"),
        "output: {output}"
    );
}

#[test]
fn variable_and_array_named_p_still_work() {
    let (result, output) = run_lines(&["10 P=2:DIM P(2):P(1)=3:PRINT P*P(1)"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "6\n");
}
