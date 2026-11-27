//! Tests that RENUMBER gives the lines new numbers and corrects each jump (issue #30).

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

const PROGRAM: &str = "5 GOTO 100\n7 PRINT \"SKIPPED\"\n100 PRINT \"DONE\"\n";

#[test]
fn test_renumber_with_defaults_lists_10_20_30_and_goto_30() {
    let output = run_binary(&format!("{PROGRAM}RENUMBER\nLIST\nEXIT\n"));

    assert!(
        output.contains("10 GOTO 30\n20 PRINT \"SKIPPED\"\n30 PRINT \"DONE\"\n"),
        "output: {output}"
    );
    assert!(!output.contains("Error"), "output: {output}");
}

#[test]
fn test_renumbered_program_runs_as_before() {
    let before = run_binary(&format!("{PROGRAM}RUN\nEXIT\n"));
    let after = run_binary(&format!("{PROGRAM}RENUMBER\nRUN\nEXIT\n"));

    assert!(before.contains("DONE\n"), "output: {before}");
    assert!(!before.contains("SKIPPED\n"), "output: {before}");
    assert_eq!(after.replace("> ", ""), before.replace("> ", ""));
}

#[test]
fn test_renumber_with_start_and_step() {
    let output = run_binary(&format!("{PROGRAM}RENUMBER 100, 5\nLIST\nEXIT\n"));

    assert!(
        output.contains("100 GOTO 110\n105 PRINT \"SKIPPED\"\n110 PRINT \"DONE\"\n"),
        "output: {output}"
    );
}

#[test]
fn test_renumber_with_start_only_uses_step_10() {
    let output = run_binary(&format!("{PROGRAM}renumber 1000\nLIST\nEXIT\n"));

    assert!(
        output.contains("1000 GOTO 1020\n1010 PRINT \"SKIPPED\"\n1020 PRINT \"DONE\"\n"),
        "output: {output}"
    );
}

#[test]
fn test_missing_target_is_reported_with_the_new_line_number() {
    let output = run_binary("5 PRINT 1\n7 GOTO 99\nRENUMBER\nLIST\nEXIT\n");

    assert!(output.contains("Failed at 20\n"), "output: {output}");
    assert!(output.contains("20 GOTO 99\n"), "output: {output}");
}

#[test]
fn test_line_number_too_big_changes_nothing() {
    let output = run_binary(&format!("{PROGRAM}RENUMBER 65530\nLIST\nEXIT\n"));

    assert!(
        output.contains("Error: Line number too big\n"),
        "output: {output}"
    );
    assert!(output.contains("5 GOTO 100\n"), "output: {output}");
}
