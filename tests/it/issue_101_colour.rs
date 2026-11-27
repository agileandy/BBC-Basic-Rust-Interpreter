//! Issue #101: COLOUR sets the text foreground or background colour.

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;

/// Store the source lines, run the program and return the result and the
/// executor, so that a test can read the text colours.
fn run_executor(source: &[&str]) -> (Result<(), String>, Executor) {
    let mut executor = Executor::new();
    let mut program = ProgramStore::new();
    for line in source {
        program.store_line(tokenize(line).expect("line tokenizes"));
    }
    let result = run_program(&mut executor, &mut program);
    (result, executor)
}

/// Store the source lines, run the program and return the result and the output.
fn run(source: &[&str]) -> (Result<(), String>, String) {
    let (result, executor) = run_executor(source);
    (result, executor.get_output().to_string())
}

#[test]
fn test_colour_sets_the_text_foreground() {
    let (result, executor) = run_executor(&["10 MODE 1 : COLOUR 2"]);

    assert_eq!(result, Ok(()));
    assert_eq!(executor.text_foreground(), 2);
    assert_eq!(executor.text_background(), 0);
}

#[test]
fn test_colour_from_128_sets_the_text_background() {
    let (result, executor) = run_executor(&["10 MODE 1 : COLOUR 129"]);

    assert_eq!(result, Ok(()));
    assert_eq!(executor.text_background(), 1);
    assert_eq!(executor.text_foreground(), 3);
}

#[test]
fn test_colour_above_the_colour_count_is_reduced() {
    let (result, executor) = run_executor(&["10 MODE 0 : COLOUR 3"]);

    assert_eq!(result, Ok(()));
    assert_eq!(executor.text_foreground(), 1);
}

#[test]
fn test_background_above_the_colour_count_is_reduced() {
    let (result, executor) = run_executor(&["10 MODE 1 : COLOUR 134"]);

    assert_eq!(result, Ok(()));
    assert_eq!(executor.text_background(), 2);
}

#[test]
fn test_vdu_17_gives_the_same_state_as_colour() {
    let (_, by_colour) = run_executor(&["10 MODE 2 : COLOUR 2 : COLOUR 133"]);
    let (result, by_vdu) = run_executor(&["10 MODE 2 : VDU 17, 2, 17, 133"]);

    assert_eq!(result, Ok(()));
    assert_eq!(by_vdu.text_foreground(), by_colour.text_foreground());
    assert_eq!(by_vdu.text_background(), by_colour.text_background());
    assert_eq!(by_vdu.text_foreground(), 2);
    assert_eq!(by_vdu.text_background(), 5);
}

#[test]
fn test_colour_puts_no_escape_code_in_the_output() {
    let (result, output) = run(&["10 COLOUR 1 : PRINT \"A\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "A\n");
}

#[test]
fn test_mode_sets_the_default_text_colours_again() {
    let (result, executor) = run_executor(&["10 MODE 2 : COLOUR 3 : COLOUR 130", "20 MODE 1"]);

    assert_eq!(result, Ok(()));
    assert_eq!(executor.text_foreground(), 3);
    assert_eq!(executor.text_background(), 0);
}

#[test]
fn test_colour_with_an_expression() {
    let (result, executor) = run_executor(&["10 MODE 2 : X% = 4", "20 COLOUR X% + 128"]);

    assert_eq!(result, Ok(()));
    assert_eq!(executor.text_background(), 4);
}

#[test]
fn test_colour_with_no_argument_gives_syntax_error() {
    let (result, _) = run(&["10 COLOUR"]);

    let message = result.expect_err("COLOUR with no argument must give an error");
    assert_eq!(
        message,
        "Syntax error at line 10: COLOUR requires a colour number"
    );
}
