//! Tests for issue 1: a comparison of two strings.

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

/// Run a program that prints "T" if the condition is true and "F" if it is false.
fn check(setup: &[&str], condition: &str) -> String {
    let mut source: Vec<String> = setup.iter().map(|line| line.to_string()).collect();
    source.push(format!(
        "100 IF {} THEN PRINT \"T\" ELSE PRINT \"F\"",
        condition
    ));
    let lines: Vec<&str> = source.iter().map(String::as_str).collect();
    let (result, output) = run(&lines);
    assert_eq!(result, Ok(()), "condition: {}", condition);
    output
}

const SETUP: &[&str] = &["10 A$ = \"APPLE\"", "20 B$ = \"BANANA\""];

#[test]
fn test_equal_strings() {
    assert_eq!(check(SETUP, "A$ = \"APPLE\""), "T\n");
    assert_eq!(check(SETUP, "A$ = B$"), "F\n");
    assert_eq!(check(SETUP, "\"X\" = \"X\""), "T\n");
}

#[test]
fn test_not_equal_strings() {
    assert_eq!(check(SETUP, "A$ <> B$"), "T\n");
    assert_eq!(check(SETUP, "A$ <> \"APPLE\""), "F\n");
}

#[test]
fn test_less_than_and_greater_than_strings() {
    assert_eq!(check(SETUP, "A$ < B$"), "T\n");
    assert_eq!(check(SETUP, "A$ > B$"), "F\n");
    assert_eq!(check(SETUP, "B$ > A$"), "T\n");
    // ASCII order: an upper-case letter is less than a lower-case letter
    assert_eq!(check(SETUP, "\"Z\" < \"a\""), "T\n");
}

#[test]
fn test_less_or_equal_and_greater_or_equal_strings() {
    assert_eq!(check(SETUP, "A$ <= \"APPLE\""), "T\n");
    assert_eq!(check(SETUP, "B$ <= A$"), "F\n");
    assert_eq!(check(SETUP, "B$ >= A$"), "T\n");
    assert_eq!(check(SETUP, "A$ >= B$"), "F\n");
}

#[test]
fn test_string_function_in_comparison() {
    assert_eq!(check(SETUP, "LEFT$(A$, 3) = \"APP\""), "T\n");
    assert_eq!(check(SETUP, "\"APP\" = LEFT$(A$, 3)"), "T\n");
}

#[test]
fn test_joined_string_in_comparison() {
    assert_eq!(check(SETUP, "A$ + B$ = \"APPLEBANANA\""), "T\n");
    assert_eq!(check(SETUP, "\"BANANA\" = \"BAN\" + \"ANA\""), "T\n");
}

#[test]
fn test_user_string_function_in_comparison() {
    let setup = &["10 DEF FNname$ = \"APPLE\"", "20 A$ = \"APPLE\""];
    assert_eq!(check(setup, "FNname$ = A$"), "T\n");
}

#[test]
fn test_until_with_string_comparison() {
    let (result, output) = run(&[
        "10 A$ = \"\"",
        "20 REPEAT",
        "30 A$ = A$ + \"X\"",
        "40 UNTIL A$ = \"XXX\"",
        "50 PRINT A$",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "XXX\n");
}

#[test]
fn test_string_compared_with_number_gives_type_mismatch() {
    let (result, output) = run(&["10 A$ = \"X\"", "20 IF A$ = 5 THEN PRINT \"T\""]);

    assert_eq!(result, Err("Type mismatch at line 20".to_string()));
    assert_eq!(output, "");
}

#[test]
fn test_number_compared_with_string_gives_type_mismatch() {
    let (result, _) = run(&["10 A$ = \"X\"", "20 IF 5 = A$ THEN PRINT \"T\""]);

    assert_eq!(result, Err("Type mismatch at line 20".to_string()));
}

#[test]
fn test_numeric_comparison_still_works() {
    assert_eq!(check(&["10 X = 1.5"], "X > 1"), "T\n");
    assert_eq!(check(&["10 X% = 2"], "X% = 2"), "T\n");
    assert_eq!(check(&["10 X% = 2"], "X% <> 3"), "T\n");
    assert_eq!(check(&["10 X% = 2"], "X% <> 2"), "F\n");
}
