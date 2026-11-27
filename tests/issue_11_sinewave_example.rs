//! Issue #11: the sine wave example must print a wave, not a straight line.
//!
//! This test runs the real binary. Outside `cfg(test)`, `TAB` writes its
//! spaces only to stdout and not to the executor output buffer, so the
//! library output does not show the column of each star.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Run the binary with the given input lines and return its stdout.
/// Kill the binary and fail if it runs for more than 10 seconds.
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

    let deadline = Instant::now() + Duration::from_secs(10);
    while child.try_wait().expect("child status").is_none() {
        if Instant::now() > deadline {
            child.kill().expect("child is killed");
            panic!("the binary ran for more than 10 seconds");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    reader.join().expect("reader thread ends")
}

#[test]
fn test_sinewave_example_prints_stars_in_a_wave() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/sinewave.bas");
    let source = std::fs::read_to_string(&path).expect("example is readable");
    let mut input: String = source
        .lines()
        .filter(|line| line.trim_start().starts_with(|c: char| c.is_ascii_digit()))
        .map(|line| format!("{}\n", line))
        .collect();
    input.push_str("RUN\nEXIT\n");

    let output = run_binary(&input);
    assert!(
        output.contains("Wave animation complete!"),
        "the program did not complete:\n{}",
        output
    );

    // The column of each star is the number of spaces before it
    let columns: Vec<usize> = output
        .lines()
        .filter(|line| line.trim() == "*")
        .map(|line| line.len() - line.trim_start().len())
        .collect();

    assert!(!columns.is_empty(), "no stars in output:\n{}", output);
    let mut distinct = columns.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert!(
        distinct.len() >= 5,
        "stars are in only {} columns: {:?}",
        distinct.len(),
        distinct
    );
    assert!(
        columns.iter().all(|&c| (5..=35).contains(&c)),
        "a star is outside columns 5 to 35: {:?}",
        distinct
    );
}
