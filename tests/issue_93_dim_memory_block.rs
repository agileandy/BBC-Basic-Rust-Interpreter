//! Issue 93: DIM with no brackets reserves a block of memory, and a memory
//! write can use an array element as the base.

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
fn test_block_can_be_written_and_read() {
    let (result, output) = run(&["10 DIM B% 10", "20 ?B% = 65", "30 PRINT ?B%"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "65\n");
}

#[test]
fn test_two_blocks_do_not_overlap() {
    let (result, output) = run(&["10 DIM A% 10, B% 20", "20 PRINT B% - A%"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "11\n");
}

#[test]
fn test_block_is_not_below_top_and_is_below_himem() {
    let (result, output) = run(&[
        "10 DIM A% 10",
        "20 PRINT A% >= TOP",
        "30 PRINT A% + 10 < HIMEM",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "-1\n-1\n");
}

#[test]
fn test_block_mixed_with_array() {
    let (result, output) = run(&[
        "10 DIM A(5), B% 10",
        "20 A(5) = 3",
        "30 ?B% = 7",
        "40 PRINT A(5) + ?B%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "10\n");
}

#[test]
fn test_block_that_is_too_large_gives_dim_space_error() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 DIM A% 40000",
        "30 END",
        "100 PRINT ERR",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "11\n");
}

#[test]
fn test_block_of_minus_one_reserves_no_space() {
    let (result, output) = run(&["10 DIM A% -1", "20 DIM B% 5", "30 PRINT B% - A%"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0\n");
}

#[test]
fn test_block_size_below_minus_one_gives_bad_dim_error() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 DIM A% -2",
        "30 END",
        "100 PRINT ERR",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "10\n");
}

#[test]
fn test_second_run_gives_same_address() {
    let mut executor = Executor::new();
    let mut program = ProgramStore::new();
    for line in ["10 DIM A% 10", "20 PRINT A%"] {
        program.store_line(tokenize(line).expect("line tokenizes"));
    }

    assert_eq!(run_program(&mut executor, &mut program), Ok(()));
    assert_eq!(run_program(&mut executor, &mut program), Ok(()));

    let output = executor.get_output().to_string();
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], lines[1]);
}

#[test]
fn test_clear_frees_the_blocks() {
    let (result, output) = run(&[
        "10 DIM A% 10",
        "20 X% = A%",
        "30 CLEAR",
        "40 DIM B% 10",
        "50 PRINT B% = TOP",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "-1\n");
}

#[test]
fn test_memory_write_with_array_element_as_base() {
    let (result, output) = run(&[
        "10 DIM C%(2), M% 20",
        "20 C%(1) = M%",
        "30 C%(1)?2 = 5",
        "40 C%(1)!4 = 7",
        "50 PRINT C%(1)?2",
        "60 PRINT C%(1)!4",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "5\n7\n");
}
