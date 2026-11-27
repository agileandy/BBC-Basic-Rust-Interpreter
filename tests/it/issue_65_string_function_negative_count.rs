//! Issue 65: LEFT$, RIGHT$, MID$ and STRING$ with a negative or zero count
//! give the result of the BBC BASIC rule, and never stop the interpreter.

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
/// a panic or a very large string fails the test and does not stop the test run.
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

/// Print the expression between two bars and return the output.
fn print_of(expression: &str) -> String {
    let line = format!("10 PRINT \"|\";{expression};\"|\"");
    let (result, output) = run_with_limit(&[&line]);
    assert_eq!(result, Ok(()), "{expression} gives no error");
    output
}

#[test]
fn test_left_with_minus_one_gives_full_text() {
    assert_eq!(print_of("LEFT$(\"ABC\", -1)"), "|ABC|\n");
}

#[test]
fn test_left_with_zero_gives_empty_text() {
    assert_eq!(print_of("LEFT$(\"ABC\", 0)"), "||\n");
}

#[test]
fn test_right_with_minus_one_gives_full_text() {
    assert_eq!(print_of("RIGHT$(\"ABC\", -1)"), "|ABC|\n");
}

#[test]
fn test_right_with_zero_gives_empty_text() {
    assert_eq!(print_of("RIGHT$(\"ABC\", 0)"), "||\n");
}

#[test]
fn test_mid_with_minus_one_start_gives_empty_text() {
    assert_eq!(print_of("MID$(\"ABC\", -1)"), "||\n");
}

#[test]
fn test_mid_with_minus_one_length_gives_rest_of_text() {
    assert_eq!(print_of("MID$(\"ABC\", 2, -1)"), "|BC|\n");
}

#[test]
fn test_mid_with_zero_length_gives_empty_text() {
    assert_eq!(print_of("MID$(\"ABC\", 2, 0)"), "||\n");
}

#[test]
fn test_string_with_zero_count_gives_empty_text() {
    assert_eq!(print_of("STRING$(0, \"A\")"), "||\n");
}

#[test]
fn test_string_with_minus_one_count_gives_empty_text() {
    assert_eq!(print_of("STRING$(-1, \"A\")"), "||\n");
}

#[test]
fn test_string_with_255_count_gives_255_characters() {
    let expected = format!("|{}|\n", "A".repeat(255));
    assert_eq!(print_of("STRING$(255, \"A\")"), expected);
}

#[test]
fn test_string_with_very_large_count_gives_string_too_long() {
    let (result, output) = run_with_limit(&[
        "10 ON ERROR GOTO 100",
        "20 PRINT STRING$(100000000, \"A\")",
        "30 END",
        "100 PRINT ERR",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "19");
}
