//! Issue 19: INPUT LINE stores the full reply in a string variable.

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::{detokenize, tokenize};

/// Store the source lines, give the replies to INPUT, run the program and return
/// the result and the output.
fn run_with_replies(source: &[&str], replies: &[&str]) -> (Result<(), String>, String) {
    let mut executor = Executor::new();
    for reply in replies {
        executor.queue_input(reply);
    }
    let mut program = ProgramStore::new();
    for line in source {
        program.store_line(tokenize(line).expect("line tokenizes"));
    }
    let result = run_program(&mut executor, &mut program);
    (result, executor.get_output().to_string())
}

#[test]
fn test_input_line_keeps_commas_and_spaces() {
    let (result, output) = run_with_replies(
        &["10 INPUT LINE A$", "20 PRINT \"[\";A$;\"]\""],
        &["  Hello, World,  \"quoted\" , end  "],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "?\n[  Hello, World,  \"quoted\" , end  ]\n");
}

#[test]
fn test_input_line_removes_only_the_line_end() {
    let (result, output) = run_with_replies(
        &["10 INPUT LINE A$", "20 PRINT \"[\";A$;\"]\""],
        &[" a, b \r\n"],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "?\n[ a, b ]\n");
}

#[test]
fn test_input_line_empty_reply_gives_empty_string() {
    let (result, output) = run_with_replies(
        &[
            "10 A$ = \"OLD\"",
            "20 INPUT LINE A$",
            "30 PRINT \"[\";A$;\"]\"",
        ],
        &[""],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "?\n[]\n");
}

#[test]
fn test_input_line_reads_one_line_for_each_variable() {
    let (result, output) = run_with_replies(
        &["10 INPUT LINE A$, B$", "20 PRINT A$", "30 PRINT B$"],
        &["one, two", "three"],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "?\n?\none, two\nthree\n");
}

#[test]
fn test_input_line_numeric_variable_gives_type_mismatch() {
    let (result, _) = run_with_replies(&["10 INPUT LINE A"], &["12"]);
    assert_eq!(
        result,
        Err("Runtime error at line 10: TypeMismatch".to_string())
    );

    let (result, _) = run_with_replies(&["10 INPUT LINE A%"], &["12"]);
    assert_eq!(
        result,
        Err("Runtime error at line 10: TypeMismatch".to_string())
    );
}

#[test]
fn test_plain_input_still_trims_the_reply() {
    let (result, output) =
        run_with_replies(&["10 INPUT A$", "20 PRINT \"[\";A$;\"]\""], &["  Hello  "]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "?\n[Hello]\n");
}

#[test]
fn test_list_shows_input_line_as_typed() {
    let line = tokenize("10 INPUT LINE A$").expect("line tokenizes");
    assert_eq!(
        detokenize(&line).expect("line detokenizes"),
        "10 INPUT LINE A$"
    );
}
