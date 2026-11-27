//! Tests that each DATA value is read one time in a program run (issue #56).

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

/// Assert that the run stopped with "Out of DATA" at the given line.
fn assert_out_of_data_at(result: Result<(), String>, line: u16) {
    let error = result.expect_err("READ past the last DATA value fails");
    assert!(error.contains(&format!("line {line}:")), "error: {error}");
    assert!(error.contains("Out of DATA"), "error: {error}");
}

#[test]
fn test_second_read_of_one_data_value_gives_out_of_data() {
    let (result, output) = run(&["10 DATA 1", "20 READ A%", "30 READ B%", "40 PRINT A%;B%"]);

    assert_out_of_data_at(result, 30);
    assert_eq!(output, "");
}

#[test]
fn test_data_before_between_and_after_reads_gives_each_value_one_time() {
    let (result, output) = run(&[
        "10 DATA 1",
        "20 READ A%",
        "30 DATA 2",
        "40 READ B%",
        "50 READ C%",
        "60 PRINT A%;B%;C%",
        "70 DATA 3",
        "80 READ D%",
    ]);

    assert_out_of_data_at(result, 80);
    assert_eq!(output, "123\n");
}

#[test]
fn test_loop_through_data_line_three_times_reads_each_value_one_time() {
    // The FOR loop has a fixed count, so a defect gives a wrong output, not a hang
    let (result, output) = run(&[
        "10 FOR I% = 1 TO 3",
        "20 DATA 7, 8, 9",
        "30 READ A%",
        "40 PRINT A%",
        "50 NEXT I%",
        "60 READ B%",
    ]);

    assert_out_of_data_at(result, 60);
    assert_eq!(output, "7\n8\n9\n");
}

#[test]
fn test_restore_then_read_gives_first_value_again() {
    let (result, output) = run(&[
        "10 DATA 5, 6",
        "20 READ A%",
        "30 RESTORE",
        "40 READ B%, C%",
        "50 PRINT A%;B%;C%",
        "60 READ D%",
    ]);

    assert_out_of_data_at(result, 60);
    assert_eq!(output, "556\n");
}
