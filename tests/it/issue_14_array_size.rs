//! Issue 14: DIM A(N) makes the elements 0 to N in each dimension.

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
fn test_real_array_holds_index_0_to_top_index() {
    let (result, output) = run(&[
        "10 DIM A(2)",
        "20 A(0) = 1.5",
        "30 A(1) = 2.5",
        "40 A(2) = 3.5",
        "50 PRINT A(0);A(1);A(2)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1.52.53.5\n");
}

#[test]
fn test_real_array_index_above_top_index_gives_subscript_error() {
    let (result, _) = run(&["10 DIM A(2)", "20 A(3) = 1"]);

    assert_eq!(result, Err("Subscript at line 20".to_string()));
}

#[test]
fn test_integer_array_holds_index_0_to_top_index() {
    let (result, output) = run(&[
        "10 DIM B%(2)",
        "20 B%(0) = 2",
        "30 B%(1) = 3",
        "40 B%(2) = 4",
        "50 PRINT B%(0);B%(1);B%(2)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "234\n");
}

#[test]
fn test_integer_array_index_above_top_index_gives_subscript_error() {
    let (result, _) = run(&["10 DIM B%(2)", "20 B%(3) = 1"]);

    assert_eq!(result, Err("Subscript at line 20".to_string()));
}

#[test]
fn test_string_array_holds_index_0_to_top_index() {
    let (result, output) = run(&[
        "10 DIM N$(2)",
        "20 N$(0) = \"AB\"",
        "30 N$(1) = \"CD\"",
        "40 N$(2) = \"EF\"",
        "50 PRINT N$(0);N$(1);N$(2)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "ABCDEF\n");
}

#[test]
fn test_string_array_index_above_top_index_gives_subscript_error() {
    let (result, _) = run(&["10 DIM N$(2)", "20 N$(3) = \"X\""]);

    assert_eq!(result, Err("Subscript at line 20".to_string()));
}

#[test]
fn test_two_dimension_array_holds_top_index_in_each_dimension() {
    let (result, output) = run(&["10 DIM M(2, 3)", "20 M(2, 3) = 7", "30 PRINT M(2, 3)"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "7\n");
}

#[test]
fn test_two_dimension_array_first_index_above_top_gives_subscript_error() {
    let (result, _) = run(&["10 DIM M(2, 3)", "20 M(3, 0) = 1"]);

    assert_eq!(result, Err("Subscript at line 20".to_string()));
}

#[test]
fn test_two_dimension_array_second_index_above_top_gives_subscript_error() {
    let (result, _) = run(&["10 DIM M(2, 3)", "20 M(0, 4) = 1"]);

    assert_eq!(result, Err("Subscript at line 20".to_string()));
}

#[test]
fn test_dim_zero_makes_one_element() {
    let (result, output) = run(&["10 DIM A%(0)", "20 A%(0) = 5", "30 PRINT A%(0)"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "5\n");

    let (result, _) = run(&["10 DIM A%(0)", "20 A%(1) = 5"]);
    assert_eq!(result, Err("Subscript at line 20".to_string()));
}

#[test]
fn test_negative_dim_size_gives_error() {
    let (result, _) = run(&["10 DIM A(-1)"]);

    assert!(result.is_err(), "DIM A(-1) must fail, got {result:?}");
}
