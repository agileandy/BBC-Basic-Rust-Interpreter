//! Issue 36: a number literal with an exponent gives its full value.

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;

/// Store the source lines, run the program and return the result and the output.
fn run(source: &[&str]) -> (Result<(), String>, String) {
    let mut executor = Executor::new();
    let mut program = ProgramStore::new();
    for line in source {
        program.store_line(tokenize(line).expect("line tokenizes"));
    }
    let result = run_program(&mut executor, &mut program);
    (result, executor.get_output().to_string())
}

#[test]
fn test_exponent_literal_gives_full_value() {
    let (result, output) = run(&["10 X = 1E3", "20 IF X = 1000 THEN PRINT \"OK\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "OK\n");
}

#[test]
fn test_exponent_literal_with_negative_exponent() {
    let (result, output) = run(&["10 X = 1.5E-2", "20 IF X = 0.015 THEN PRINT \"OK\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "OK\n");
}

#[test]
fn test_integer_literal_too_large_for_integer_is_real() {
    let (result, output) = run(&["10 X = 3000000000", "20 IF X = 3E9 THEN PRINT \"OK\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "OK\n");
}

#[test]
fn test_exponent_literal_assigns_to_integer_variable() {
    let (result, output) = run(&["10 C% = 2E2", "20 IF C% = 200 THEN PRINT \"OK\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "OK\n");
}

#[test]
fn test_variable_named_e_after_number_still_works() {
    let (result, output) = run(&["10 E = 5", "20 A = 2 * E", "30 IF A = 10 THEN PRINT \"OK\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "OK\n");
}
