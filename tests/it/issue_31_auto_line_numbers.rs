//! Tests that AUTO gives each typed line the next line number (issue #31).

use std::io::{Read, Write};
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

/// Run the binary with the given input and return its exit status and its stdout.
/// Kill the binary and fail if it runs for more than 5 seconds.
fn run_binary(input: &str) -> (ExitStatus, String) {
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
    let status = loop {
        if let Some(status) = child.try_wait().expect("child status") {
            break status;
        }
        if Instant::now() > deadline {
            child.kill().expect("child is killed");
            panic!("the binary ran for more than 5 seconds");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    (status, reader.join().expect("reader thread ends"))
}

#[test]
fn test_auto_numbers_lines_10_20_30() {
    let (_, output) = run_binary("AUTO\nPRINT 1\nPRINT 2\nPRINT 3\n\nLIST\nEXIT\n");

    assert!(output.contains("10 20 30 40 > "), "output: {output}");
    assert!(
        output.contains("10 PRINT 1\n20 PRINT 2\n30 PRINT 3\n"),
        "output: {output}"
    );
    assert!(!output.contains("Error"), "output: {output}");
}

#[test]
fn test_auto_with_start_and_step() {
    let (_, output) = run_binary("AUTO 100, 5\nPRINT 1\nPRINT 2\n\nLIST\nEXIT\n");

    assert!(output.contains("100 105 110 > "), "output: {output}");
    assert!(
        output.contains("100 PRINT 1\n105 PRINT 2\n"),
        "output: {output}"
    );
}

#[test]
fn test_auto_with_start_only_uses_step_10() {
    let (_, output) = run_binary("AUTO 500\nPRINT 1\nPRINT 2\n\nLIST\nEXIT\n");

    assert!(
        output.contains("500 PRINT 1\n510 PRINT 2\n"),
        "output: {output}"
    );
}

#[test]
fn test_auto_replaces_a_line_that_exists() {
    let (_, output) = run_binary("10 PRINT \"OLD\"\nAUTO\nPRINT \"NEW\"\n\nLIST\nEXIT\n");

    assert!(output.contains("10 PRINT \"NEW\"\n"), "output: {output}");
    assert!(!output.contains("OLD"), "output: {output}");
}

#[test]
fn test_auto_ends_with_too_big_above_65535() {
    let (_, output) = run_binary("AUTO 65530, 5\nPRINT 1\nPRINT 2\nLIST\nEXIT\n");

    assert!(
        output.contains("65530 65535 Error: Too big\n"),
        "output: {output}"
    );
    // The third line is a typed command again, so LIST runs.
    assert!(
        output.contains("65530 PRINT 1\n65535 PRINT 2\n"),
        "output: {output}"
    );
}

#[test]
fn test_auto_start_above_65535_gives_too_big() {
    let (_, output) = run_binary("AUTO 70000\nLIST\nEXIT\n");

    assert!(output.contains("Error: Too big\n"), "output: {output}");
    assert!(output.contains("No program"), "output: {output}");
}

#[test]
fn test_auto_step_0_gives_silly() {
    let (_, output) = run_binary("AUTO 10, 0\nLIST\nEXIT\n");

    assert!(output.contains("Error: Silly\n"), "output: {output}");
    assert!(output.contains("No program"), "output: {output}");
}

#[test]
fn test_auto_bad_argument_gives_syntax_error() {
    let (_, output) = run_binary("AUTO 10, X\nLIST\nEXIT\n");

    assert!(output.contains("Error: Syntax error\n"), "output: {output}");
    assert!(output.contains("No program"), "output: {output}");
}

#[test]
fn test_end_of_input_in_auto_mode_ends_with_status_0() {
    let (status, output) = run_binary("AUTO\nPRINT 1\n");

    assert!(status.success(), "status: {status}, output: {output}");
    assert!(output.contains("10 20 \n"), "output: {output}");
}

#[test]
fn test_help_names_auto() {
    let (_, output) = run_binary("HELP\nEXIT\n");

    assert!(output.contains("AUTO [start[,step]]"), "output: {output}");
}
