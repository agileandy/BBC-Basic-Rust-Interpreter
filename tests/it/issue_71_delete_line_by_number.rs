//! Tests that a line number typed alone deletes that line (issue #71).

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

const PROGRAM: &str = "10 PRINT \"ONE\"\n20 PRINT \"TWO\"\n30 PRINT \"THREE\"\n";

#[test]
fn test_line_number_alone_deletes_the_line_from_list() {
    let output = run_binary(&format!("{PROGRAM}20\nLIST\nEXIT\n"));

    assert!(output.contains("10 PRINT \"ONE\"\n"), "output: {output}");
    assert!(output.contains("30 PRINT \"THREE\"\n"), "output: {output}");
    assert!(!output.contains("TWO"), "output: {output}");
    assert!(!output.contains("Error"), "output: {output}");
}

#[test]
fn test_line_number_alone_deletes_the_line_from_run() {
    let output = run_binary(&format!("{PROGRAM}20\nRUN\nEXIT\n"));

    assert!(output.contains("ONE\n"), "output: {output}");
    assert!(output.contains("THREE\n"), "output: {output}");
    assert!(!output.contains("TWO"), "output: {output}");
}

#[test]
fn test_line_number_with_spaces_deletes_the_line() {
    let output = run_binary(&format!("{PROGRAM}  20   \nLIST\nEXIT\n"));

    assert!(output.contains("10 PRINT \"ONE\"\n"), "output: {output}");
    assert!(output.contains("30 PRINT \"THREE\"\n"), "output: {output}");
    assert!(!output.contains("TWO"), "output: {output}");
}

#[test]
fn test_number_of_missing_line_gives_no_error_text() {
    let output = run_binary(&format!("{PROGRAM}25\nLIST\nEXIT\n"));

    assert!(!output.contains("Error"), "output: {output}");
    assert!(!output.contains("deleted"), "output: {output}");
    assert!(output.contains("20 PRINT \"TWO\"\n"), "output: {output}");
}
