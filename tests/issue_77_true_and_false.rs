//! Issue #77: TRUE and FALSE with no bracket give -1 and 0.

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

/// Run the program in a separate thread with a time limit of 5 seconds, so that
/// a loop that does not end fails the test and does not stop the test run.
fn run_with_limit(source: &[&str]) -> (Result<(), String>, String) {
    let source: Vec<String> = source.iter().map(|line| line.to_string()).collect();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let lines: Vec<&str> = source.iter().map(String::as_str).collect();
        let _ = sender.send(run(&lines));
    });
    receiver
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("program ends within 5 seconds and does not panic")
}

#[test]
fn test_print_true_shows_minus_one() {
    let (result, output) = run_with_limit(&["10 PRINT TRUE"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "-1");
}

#[test]
fn test_print_false_shows_zero() {
    let (result, output) = run_with_limit(&["10 PRINT FALSE"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_if_true_runs_then_part() {
    let (result, output) = run_with_limit(&["10 IF TRUE THEN PRINT \"T\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "T\n");
}

#[test]
fn test_repeat_until_with_or_false_ends() {
    let (result, output) = run_with_limit(&[
        "10 N% = 0",
        "20 REPEAT : N% = N% + 1 : UNTIL N% = 3 OR FALSE",
        "30 PRINT N%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "3");
}

/// The OR test above passes without the fix, because OR is not parsed (a
/// separate defect). This test makes the loop condition depend on FALSE.
#[test]
fn test_repeat_until_with_and_not_false_ends() {
    let (result, output) = run_with_limit(&[
        "10 N% = 0",
        "20 REPEAT : N% = N% + 1 : UNTIL N% = 3 AND NOT FALSE",
        "30 PRINT N%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "3");
}

#[test]
fn test_true_and_five_gives_five() {
    let (result, output) = run_with_limit(&["10 A% = TRUE AND 5", "20 PRINT A%"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_not_false_variable_runs_then_part() {
    let (result, output) = run_with_limit(&["10 F% = FALSE : IF NOT F% THEN PRINT \"N\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "N\n");
}

#[test]
fn test_names_that_start_with_true_or_false_are_variables() {
    let (result, output) = run_with_limit(&[
        "10 TRUEVALUE = 5",
        "20 FALSEFLAG% = 7",
        "30 PRINT TRUEVALUE; \" \"; FALSEFLAG%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "5 7\n");
}
