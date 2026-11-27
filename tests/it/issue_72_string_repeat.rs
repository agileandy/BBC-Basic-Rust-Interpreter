//! Issue 72: STRING$(n, A$) repeats the full text A$ n times, not only its
//! first character.

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
fn test_string_repeats_full_text() {
    assert_eq!(print_of("STRING$(3, \"AB\")"), "|ABABAB|\n");
}

#[test]
fn test_string_of_empty_text_is_empty() {
    assert_eq!(print_of("STRING$(2, \"\")"), "||\n");
}

#[test]
fn test_string_with_255_characters_works() {
    let expected = format!("|{}|\n", "A".repeat(255));
    assert_eq!(print_of("STRING$(255, \"A\")"), expected);
}

#[test]
fn test_string_with_zero_count_is_empty() {
    assert_eq!(print_of("STRING$(0, \"AB\")"), "||\n");
}

#[test]
fn test_string_longer_than_65535_characters_gives_string_too_long() {
    let (result, output) = run_with_limit(&[
        "10 ON ERROR GOTO 100",
        "20 PRINT STRING$(32768, \"AB\")",
        "30 END",
        "100 PRINT ERR",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "19");
}
