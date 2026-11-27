//! Tests that DELETE start, end removes a range of lines (issue #32).

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

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
    let reader = std::thread::spawn(move || {
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
        std::thread::sleep(Duration::from_millis(20));
    }
    reader.join().expect("reader thread ends")
}

const PROGRAM: &str = "10 PRINT \"ONE\"\n20 PRINT \"TWO\"\n30 PRINT \"THREE\"\n40 PRINT \"FOUR\"\n50 PRINT \"FIVE\"\n";
const ALL_TEXT: [&str; 5] = ["ONE", "TWO", "THREE", "FOUR", "FIVE"];

#[test]
fn test_delete_range_removes_lines_from_start_to_end() {
    let output = run_binary(&format!("{PROGRAM}DELETE 20, 40\nLIST\nEXIT\n"));

    assert!(output.contains("10 PRINT \"ONE\"\n"), "output: {output}");
    assert!(output.contains("50 PRINT \"FIVE\"\n"), "output: {output}");
    assert!(!output.contains("TWO"), "output: {output}");
    assert!(!output.contains("THREE"), "output: {output}");
    assert!(!output.contains("FOUR"), "output: {output}");
    assert!(!output.contains("Error"), "output: {output}");
}

#[test]
fn test_delete_range_with_no_lines_gives_no_error() {
    let output = run_binary(&format!("{PROGRAM}DELETE 21, 29\nLIST\nEXIT\n"));

    assert!(!output.contains("Error"), "output: {output}");
    for text in ALL_TEXT {
        assert!(output.contains(text), "missing {text}, output: {output}");
    }
}

#[test]
fn test_delete_range_in_wrong_order_gives_error_and_removes_nothing() {
    let output = run_binary(&format!("{PROGRAM}DELETE 40, 20\nLIST\nEXIT\n"));

    assert!(output.contains("Error: Syntax error"), "output: {output}");
    for text in ALL_TEXT {
        assert!(output.contains(text), "missing {text}, output: {output}");
    }
}

#[test]
fn test_delete_with_one_number_gives_error_and_removes_nothing() {
    let output = run_binary(&format!("{PROGRAM}DELETE 20\nLIST\nEXIT\n"));

    assert!(output.contains("Error: Syntax error"), "output: {output}");
    for text in ALL_TEXT {
        assert!(output.contains(text), "missing {text}, output: {output}");
    }
}

#[test]
fn test_delete_in_lower_case_removes_the_range() {
    let output = run_binary(&format!("{PROGRAM}delete 10,20\nLIST\nEXIT\n"));

    assert!(!output.contains("ONE"), "output: {output}");
    assert!(!output.contains("TWO"), "output: {output}");
    assert!(output.contains("30 PRINT \"THREE\"\n"), "output: {output}");
    assert!(!output.contains("Error"), "output: {output}");
}
