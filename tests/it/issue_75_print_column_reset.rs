//! Issue #75: the print column is 0 after an INPUT reply and at the start of a run.

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::{run_program, run_typed_line};
use bbc_basic_interpreter::tokenizer::tokenize;

/// Store the source lines in a new program store.
fn program_of(source: &[&str]) -> ProgramStore {
    let mut program = ProgramStore::new();
    for line in source {
        program.store_line(tokenize(line).expect("line tokenizes"));
    }
    program
}

/// Store the source lines, run the program with the queued replies and
/// return the result and the output.
fn run_with_replies(source: &[&str], replies: &[&str]) -> (Result<(), String>, String) {
    let mut executor = Executor::new();
    for reply in replies {
        executor.queue_input(reply);
    }
    let mut program = program_of(source);
    let result = run_program(&mut executor, &mut program);
    (result, executor.get_output().to_string())
}

#[test]
fn test_tab_after_input_counts_from_column_0() {
    let (result, output) = run_with_replies(
        &["10 PRINT \"AB\";", "20 INPUT A", "30 PRINT TAB(5);\"X\""],
        &["1"],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "AB?\n     X\n");
}

#[test]
fn test_second_run_starts_at_column_0() {
    let mut executor = Executor::new();
    let mut first = program_of(&["10 PRINT \"AB\";"]);
    assert_eq!(run_program(&mut executor, &mut first), Ok(()));
    executor.clear_output();

    let mut second = program_of(&["10 PRINT TAB(3);\"Y\""]);
    assert_eq!(run_program(&mut executor, &mut second), Ok(()));

    assert_eq!(executor.get_output(), "   Y\n");
}

#[test]
fn test_typed_line_starts_at_column_0() {
    let mut executor = Executor::new();
    let program = ProgramStore::new();
    let first = tokenize("PRINT \"AB\";").expect("line tokenizes");
    assert!(run_typed_line(&mut executor, &program, &first).is_ok());
    executor.clear_output();

    let second = tokenize("PRINT TAB(3);\"Y\"").expect("line tokenizes");
    assert!(run_typed_line(&mut executor, &program, &second).is_ok());

    assert_eq!(executor.get_output(), "   Y\n");
}
