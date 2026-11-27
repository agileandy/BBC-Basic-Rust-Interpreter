//! Issue 21: INKEY and INKEY$ wait for a key for a maximum time.

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

/// Run the real binary with piped input and return the output and the time.
/// If `close_input` is true, close stdin after the input, so that the
/// interpreter sees end of input. If it is false, keep stdin open until the
/// process stops. Fail if the process does not stop in 5 seconds.
fn run_binary(input: &str, close_input: bool) -> (String, Duration) {
    let start = Instant::now();
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

    let mut stdin = child.stdin.take().expect("stdin is piped");
    stdin.write_all(input.as_bytes()).expect("input is written");
    let mut open_stdin = if close_input {
        drop(stdin);
        None
    } else {
        Some(stdin)
    };

    let deadline = start + Duration::from_secs(5);
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
        thread::sleep(Duration::from_millis(10));
    }
    let elapsed = start.elapsed();
    open_stdin.take();

    (reader.join().expect("reader thread completes"), elapsed)
}

#[test]
fn test_inkey_gives_queued_key_with_no_wait() {
    let start = Instant::now();
    let (result, output) = run_with_keys("A", &["10 A% = INKEY(100)", "20 PRINT A%"]);

    assert!(start.elapsed() < Duration::from_millis(500));
    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "65");
}

#[test]
fn test_inkey_string_with_zero_time_gives_queued_key() {
    let (result, output) = run_with_keys("Q", &["10 A$ = INKEY$(0)", "20 PRINT \"[\"; A$; \"]\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "[Q]\n");
}

#[test]
fn test_inkey_reads_queued_keys_in_order() {
    let (result, output) = run_with_keys(
        "XY",
        &["10 A$ = INKEY$(10)", "20 B% = INKEY(10)", "30 PRINT A$; B%"],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "X89");
}

#[test]
fn test_inkey_with_no_key_gives_minus_one() {
    // The queue is empty and the time limit is 0, so the read does not wait.
    let (result, output) = run_with_keys("", &["10 A% = INKEY(0)", "20 PRINT A%"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "-1");
}

#[test]
fn test_inkey_string_with_no_key_gives_empty_string() {
    let (result, output) = run_with_keys("", &["10 A$ = INKEY$(0)", "20 PRINT \"[\"; A$; \"]\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "[]\n");
}

#[test]
fn test_inkey_with_negative_argument_gives_false_and_keeps_queue() {
    // A key scan is not possible in this build, so INKEY(-99) gives FALSE.
    // INKEY(-256) also gives 0. The queued key stays for the next read.
    let (result, output) = run_with_keys(
        "Z",
        &[
            "10 A% = INKEY(-99)",
            "20 B% = INKEY(-256)",
            "30 C$ = INKEY$(0)",
            "40 PRINT A%; \" \"; B%; \" \"; C$",
        ],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "0 0 Z");
}

#[test]
fn test_inkey_in_a_condition() {
    let (result, output) = run_with_keys(
        "N",
        &[
            "10 IF INKEY(10) = 78 THEN PRINT \"YES\" ELSE PRINT \"NO\"",
            "20 IF INKEY(0) = -1 THEN PRINT \"NONE\"",
        ],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "YES\nNONE\n");
}

#[test]
fn test_binary_inkey_with_zero_time_at_end_of_input_gives_minus_one() {
    let (output, _) = run_binary("10 A% = INKEY(0) : PRINT \"[\"; A%; \"]\"\nRUN\n", true);

    assert!(output.contains("[-1]"), "output was {output:?}");
}

#[test]
fn test_binary_inkey_at_end_of_input_does_not_wait_the_full_time() {
    let (output, elapsed) = run_binary("10 A% = INKEY(500) : PRINT \"[\"; A%; \"]\"\nRUN\n", true);

    assert!(output.contains("[-1]"), "output was {output:?}");
    assert!(elapsed < Duration::from_secs(1), "took {elapsed:?}");
}

#[test]
fn test_binary_inkey_string_reads_one_key_from_piped_input() {
    let (output, _) = run_binary(
        "10 A$ = INKEY$(500) : PRINT \"[\"; A$; \"]\"\nRUN\nXEXIT\n",
        true,
    );

    assert!(output.contains("[X]"), "output was {output:?}");
}

#[test]
fn test_binary_inkey_reads_a_key_from_an_open_pipe_with_no_wait() {
    // The key is in the input buffer and stdin stays open.
    let (output, elapsed) = run_binary(
        "10 A% = INKEY(300) : PRINT \"[\"; A%; \"]\"\n20 QUIT\nRUN\nX",
        false,
    );

    assert!(output.contains("[88]"), "output was {output:?}");
    assert!(elapsed < Duration::from_secs(2), "took {elapsed:?}");
}

#[test]
fn test_binary_inkey_on_an_open_empty_pipe_waits_then_gives_minus_one() {
    // No key comes and stdin stays open, so the read waits the time limit.
    let (output, elapsed) = run_binary(
        "10 A% = INKEY(50) : PRINT \"[\"; A%; \"]\"\n20 QUIT\nRUN\n",
        false,
    );

    assert!(output.contains("[-1]"), "output was {output:?}");
    assert!(elapsed >= Duration::from_millis(500), "took {elapsed:?}");
}
