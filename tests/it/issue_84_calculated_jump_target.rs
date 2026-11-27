//! Issue 84: GOTO and GOSUB with a calculated line number.
//!
//! The BBC BASIC rule: the target of GOTO or GOSUB is a line number constant,
//! or an expression that does not start with a digit, for example
//! `GOTO (100 + X)` or `GOSUB X * 10`. A constant followed by more tokens,
//! such as `GOTO 100 + X`, is a syntax error.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;

/// Store the source lines, run the program and return the result and the output.
///
/// The program runs in a thread with a time limit, so a jump defect that
/// loops without end fails the test and does not hang the test run.
fn run(source: &[&str]) -> (Result<(), String>, String) {
    let lines: Vec<String> = source.iter().map(|line| line.to_string()).collect();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut executor = Executor::new();
        let mut program = ProgramStore::new();
        for line in &lines {
            program.store_line(tokenize(line).expect("line tokenizes"));
        }
        let result = run_program(&mut executor, &mut program);
        let _ = sender.send((result, executor.get_output().to_string()));
    });
    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("program stops in 5 seconds")
}

/// Run the binary with the given input lines and return its stdout.
/// Kill the binary and fail if it runs for more than 5 seconds.
fn run_binary(input: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_bbc-basic-interpreter"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("binary starts");
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(input.as_bytes())
        .expect("input is written");

    let mut stdout = child.stdout.take().expect("stdout is piped");
    let reader = thread::spawn(move || {
        let mut text = String::new();
        stdout
            .read_to_string(&mut text)
            .expect("stdout is readable");
        text
    });

    let deadline = Instant::now() + Duration::from_secs(5);
    while child.try_wait().expect("child status").is_none() {
        if Instant::now() > deadline {
            child.kill().expect("child is killed");
            panic!("the binary ran for more than 5 seconds");
        }
        thread::sleep(Duration::from_millis(20));
    }
    reader.join().expect("reader thread ends")
}

#[test]
fn test_jump_table_with_calculated_goto() {
    for (n, expected) in [(0, "zero\n"), (1, "one\n"), (2, "two\n")] {
        let line_10 = format!("10 N% = {}", n);
        let (result, output) = run(&[
            &line_10,
            "20 GOTO (100 + N% * 10)",
            "30 PRINT \"fell through\"",
            "40 END",
            "100 PRINT \"zero\" : END",
            "110 PRINT \"one\" : END",
            "120 PRINT \"two\" : END",
        ]);

        assert_eq!(result, Ok(()), "N%={}", n);
        assert_eq!(output, expected, "N%={}", n);
    }
}

#[test]
fn test_gosub_to_variable_returns() {
    let (result, output) = run(&[
        "10 L% = 100",
        "20 GOSUB L%",
        "30 PRINT \"back\"",
        "40 END",
        "100 PRINT \"sub\"",
        "110 RETURN",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "sub\nback\n");
}

#[test]
fn test_gosub_with_calculated_target_without_brackets() {
    let (result, output) = run(&[
        "10 X = 10",
        "20 GOSUB X * 10",
        "30 PRINT \"back\"",
        "40 END",
        "100 PRINT \"sub\" : RETURN",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "sub\nback\n");
}

#[test]
fn test_goto_constant_is_unchanged() {
    let (result, output) = run(&["10 GOTO 100", "20 PRINT \"skipped\"", "100 PRINT \"done\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "done\n");
}

#[test]
fn test_constant_then_more_tokens_is_syntax_error() {
    let (result, output) = run(&[
        "10 X = 5",
        "20 GOTO 100 + X",
        "100 PRINT \"jumped\"",
        "105 PRINT \"jumped to 105\"",
    ]);

    let message = result.expect_err("GOTO 100 + X is a syntax error");
    assert!(message.contains("Syntax"), "{}", message);
    assert!(message.contains("line 20"), "{}", message);
    assert_eq!(output, "");
}

#[test]
fn test_calculated_target_to_missing_line_gives_goto_error() {
    let (result, _) = run(&["10 X% = 5", "20 GOTO (100 + X%)", "100 END"]);

    assert_eq!(result, Err("No such line: 105 at line 20".to_string()));
}

#[test]
fn test_calculated_gosub_to_missing_line_gives_gosub_error() {
    let (result, _) = run(&["10 X% = 5", "20 GOSUB (100 + X%)", "100 END"]);

    assert_eq!(result, Err("No such line: 105 at line 20".to_string()));
}

#[test]
fn test_calculated_target_out_of_range_gives_goto_error() {
    for (value, shown) in [("-1", "-1"), ("70000", "70000")] {
        let line_10 = format!("10 X% = {}", value);
        let (result, _) = run(&[&line_10, "20 GOTO (X%)", "100 END"]);

        assert_eq!(result, Err(format!("No such line: {} at line 20", shown)));
    }
}

#[test]
fn test_renumber_leaves_calculated_target_unchanged() {
    let output = run_binary(
        "5 X% = 2\n7 GOTO (100 + X%)\n9 GOSUB X% * 50\n102 PRINT \"DONE\"\nRENUMBER\nLIST\nEXIT\n",
    );

    assert!(output.contains("20 GOTO (100 + X%)"), "{}", output);
    assert!(output.contains("30 GOSUB X% * 50"), "{}", output);
    assert!(output.contains("40 PRINT \"DONE\""), "{}", output);
}
