//! Issue 25: the memory values PAGE and TOP, and LOMEM equal to TOP.

use crate::common::run_lines as run;
use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_typed_line;
use bbc_basic_interpreter::tokenizer::tokenize;

/// Run a program that prints one integer value, and return that value.
fn printed_value(source: &[&str]) -> i64 {
    let (result, output) = run(source);
    assert_eq!(result, Ok(()), "output: {output}");
    output
        .trim()
        .parse()
        .unwrap_or_else(|_| panic!("not an integer: {output:?}"))
}

#[test]
fn test_page_is_not_more_than_top() {
    let difference = printed_value(&["10 PRINT TOP - PAGE"]);

    assert!(difference >= 0, "TOP - PAGE is {difference}");
}

#[test]
fn test_top_is_more_than_page_for_one_line() {
    let difference = printed_value(&["10 PRINT TOP - PAGE"]);

    assert!(difference > 0, "TOP - PAGE is {difference}");
}

#[test]
fn test_top_grows_when_a_line_is_added() {
    let two_lines = printed_value(&["10 PRINT TOP", "20 END"]);
    let three_lines = printed_value(&["10 PRINT TOP", "20 END", "30 REM MORE"]);

    assert!(
        three_lines > two_lines,
        "TOP with 3 lines is {three_lines}, with 2 lines is {two_lines}"
    );
}

#[test]
fn test_lomem_is_equal_to_top() {
    let (result, output) = run(&["10 PRINT LOMEM = TOP", "20 REM X"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "        -1\n");
}

#[test]
fn test_himem_is_more_than_top() {
    let (result, output) = run(&["10 PRINT HIMEM > TOP"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "        -1\n");
}

#[test]
fn test_print_page_shows_an_integer() {
    let (result, output) = run(&["10 PRINT PAGE"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "      6400\n");
}

#[test]
fn test_typed_line_sees_the_stored_program() {
    let mut executor = Executor::new();
    let mut program = ProgramStore::new();
    program.store_line(tokenize("10 REM A STORED LINE").expect("line tokenizes"));

    let typed = tokenize("PRINT TOP - PAGE > 2").expect("line tokenizes");
    let result = run_typed_line(&mut executor, &program, &typed);

    assert!(result.is_ok(), "result: {result:?}");
    assert_eq!(executor.get_output(), "        -1\n");
}
