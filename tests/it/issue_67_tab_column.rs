//! Issue #67: TAB and the comma in PRINT must put text at the correct column.

use crate::common::run_lines as run;
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
fn test_tab_after_text_moves_to_the_given_column() {
    let (result, output) = run(&["10 PRINT \"AB\"; TAB(10); \"C\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "AB        C\n");
}

#[test]
fn test_tab_to_a_column_before_the_cursor_starts_a_new_line() {
    let (result, output) = run(&["10 PRINT TAB(5); \"A\"; TAB(3); \"B\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "     A\n   B\n");
}

#[test]
fn test_tab_to_the_cursor_column_prints_nothing() {
    let (result, output) = run(&["10 PRINT \"ABC\"; TAB(3); \"D\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "ABCD\n");
}

#[test]
fn test_comma_moves_to_the_next_zone_of_10_columns() {
    let (result, output) = run(&["10 PRINT 1, 2"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1         2\n");
}

#[test]
fn test_comma_after_a_full_zone_moves_to_the_zone_after_it() {
    let (result, output) = run(&["10 PRINT \"ABCDEFGHIJ\", \"K\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "ABCDEFGHIJ          K\n");
}

#[test]
fn test_spc_prints_spaces_from_the_cursor() {
    let (result, output) = run(&["10 PRINT \"A\"; SPC(3); \"B\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "A   B\n");
}

#[test]
fn test_tab_on_a_new_line_counts_from_column_0() {
    let (result, output) = run(&["10 PRINT \"HELLO\"", "20 PRINT TAB(3); \"X\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "HELLO\n   X\n");
}

#[test]
fn test_comma_on_a_new_line_counts_from_column_0() {
    let (result, output) = run(&["10 PRINT \"HELLO WORLD\"", "20 PRINT 1, 2"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "HELLO WORLD\n1         2\n");
}

#[test]
fn test_tab_counts_from_text_of_an_earlier_print_with_semicolon() {
    let (result, output) = run(&["10 PRINT \"AB\";", "20 PRINT TAB(6); \"C\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "AB    C\n");
}

#[test]
fn test_tab_after_cls_counts_from_column_0() {
    let (result, output) = run(&["10 PRINT \"AB\";", "20 CLS", "30 PRINT TAB(3); \"X\""]);

    assert_eq!(result, Ok(()));
    // The library build sends the CLS escape to the screen only
    assert!(output.ends_with("AB   X\n"), "output: {:?}", output);
}

#[test]
fn test_binary_gives_the_same_text_as_the_library_buffer() {
    let source = [
        "10 PRINT \"AB\"; TAB(10); \"C\"",
        "20 PRINT TAB(5); \"A\"; TAB(3); \"B\"",
        "30 PRINT 1, 2",
        "40 PRINT \"A\"; SPC(3); \"B\"",
        "50 PRINT \"END\"",
    ];
    let (result, expected) = run(&source);
    assert_eq!(result, Ok(()));

    let mut input: String = source.iter().map(|line| format!("{}\n", line)).collect();
    input.push_str("RUN\nEXIT\n");
    let output = run_binary(&input);

    // The binary writes the program output after the prompt of the RUN line
    let start = output.find("AB").expect("program output is present");
    let end = output.find("END\n").expect("program completes") + "END\n".len();
    assert_eq!(&output[start..end], expected);
}
