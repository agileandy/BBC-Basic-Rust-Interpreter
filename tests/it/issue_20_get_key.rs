//! Issue 20: GET and GET$ wait for one key and give its code or its character.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;

/// Queue the keys, store the source lines, run the program and return the
/// result and the output.
fn run_with_keys(keys: &str, source: &[&str]) -> (Result<(), String>, String) {
    let mut executor = Executor::new();
    executor.queue_keys(keys);
    let mut program = ProgramStore::new();
    for line in source {
        program.store_line(tokenize(line).expect("line tokenizes"));
    }
    let result = run_program(&mut executor, &mut program);
    (result, executor.get_output().to_string())
}

/// Run the real binary with piped input. Fail if it does not stop in 5 seconds.
fn run_binary(input: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_bbc-basic-interpreter"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("binary starts");

    // Read stdout on a thread, so that a full pipe cannot block the child.
    let mut stdout = child.stdout.take().expect("stdout is piped");
    let reader = thread::spawn(move || {
        let mut output = String::new();
        let _ = stdout.read_to_string(&mut output);
        output
    });

    {
        let mut stdin = child.stdin.take().expect("stdin is piped");
        stdin.write_all(input.as_bytes()).expect("input is written");
        // Dropping stdin closes it, so the interpreter sees end of input.
    }

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if child
            .try_wait()
            .expect("child status is readable")
            .is_some()
        {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("interpreter did not stop within 5 seconds");
        }
        thread::sleep(Duration::from_millis(20));
    }

    reader.join().expect("reader thread completes")
}

#[test]
fn test_get_gives_the_code_of_the_key() {
    let (result, output) = run_with_keys("A", &["10 A% = GET", "20 PRINT A%"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "65");
}

#[test]
fn test_get_string_gives_the_character_of_the_key() {
    let (result, output) = run_with_keys("Q", &["10 A$ = GET$", "20 PRINT A$"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "Q\n");
}

#[test]
fn test_get_reads_three_keys_in_order() {
    let (result, output) = run_with_keys(
        "XYZ",
        &[
            "10 A$ = GET$",
            "20 B% = GET",
            "30 C$ = GET$",
            "40 PRINT A$; C$; B%",
        ],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "XZ89");
}

#[test]
fn test_get_does_not_show_the_key() {
    let (result, output) = run_with_keys("K", &["10 A% = GET"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "");
}

#[test]
fn test_binary_get_string_reads_one_key_from_piped_input() {
    let output = run_binary("10 A$ = GET$ : PRINT \"[\"; A$; \"]\"\nRUN\nXEXIT\n");

    assert!(output.contains("[X]"), "output was {output:?}");
}

#[test]
fn test_binary_get_at_end_of_input_gives_minus_one() {
    let output = run_binary("10 A% = GET : PRINT \"[\"; A%; \"]\"\nRUN\n");

    assert!(output.contains("[-1]"), "output was {output:?}");
}

#[test]
fn test_binary_get_string_at_end_of_input_gives_empty_string() {
    let output = run_binary("10 A$ = GET$ : PRINT \"[\"; A$; \"]\"\nRUN\n");

    assert!(output.contains("[]"), "output was {output:?}");
}
