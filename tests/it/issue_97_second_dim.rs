//! Issue 97: a second DIM of an array that has dimensions gives Bad DIM.

use crate::common::run_lines as run;
use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;

#[test]
fn test_second_dim_of_array_gives_bad_dim() {
    let (result, _) = run(&["10 DIM A(2) : A(1) = 5", "20 DIM A(3)"]);

    assert_eq!(result, Err("Bad DIM at line 20".to_string()));
}

#[test]
fn test_second_dim_gives_err_10_and_keeps_elements() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 40",
        "20 DIM A(2) : A(1) = 5",
        "30 DIM A(3)",
        "40 PRINT ERR",
        "50 PRINT A(1)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "        10\n         5\n");
}

#[test]
fn test_second_dim_in_one_statement_gives_bad_dim() {
    let (result, _) = run(&["10 DIM A%(2), A%(3)"]);

    assert_eq!(result, Err("Bad DIM at line 10".to_string()));
}

#[test]
fn test_local_array_can_be_dimensioned_in_each_call() {
    let (result, output) = run(&[
        "10 PROCFILL",
        "20 PROCFILL",
        "30 END",
        "100 DEF PROCFILL",
        "110 LOCAL A()",
        "120 DIM A(2)",
        "130 A(2) = 7",
        "140 PRINT A(2)",
        "150 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         7\n         7\n");
}

#[test]
fn test_second_memory_block_reserves_second_block() {
    let (result, output) = run(&[
        "10 DIM B% 10",
        "20 C% = B%",
        "30 DIM B% 10",
        "40 PRINT B% - C%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "        11\n");
}

#[test]
fn test_dim_after_clear_works() {
    let (result, output) = run(&[
        "10 DIM A(2)",
        "20 CLEAR",
        "30 DIM A(3)",
        "40 A(3) = 4",
        "50 PRINT A(3)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         4\n");
}

#[test]
fn test_second_run_of_program_with_dim_works() {
    let mut executor = Executor::new();
    let mut program = ProgramStore::new();
    for line in ["10 DIM A(2)", "20 A(2) = 3", "30 PRINT A(2)"] {
        program.store_line(tokenize(line).expect("line tokenizes"));
    }

    assert_eq!(run_program(&mut executor, &mut program), Ok(()));
    assert_eq!(run_program(&mut executor, &mut program), Ok(()));
    assert_eq!(executor.get_output(), "         3\n         3\n");
}
