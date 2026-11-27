//! Tests that QUIT in a program stops the run and leaves the interpreter (issue #18).

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::{run_program, run_program_with_outcome, RunOutcome};
use bbc_basic_interpreter::tokenizer::tokenize;

/// Store the source lines, run the program and return the result and the output.
fn run(source: &[&str]) -> (Result<RunOutcome, String>, String) {
    let mut executor = Executor::new();
    let mut program = ProgramStore::new();
    for line in source {
        program.store_line(tokenize(line).expect("line tokenizes"));
    }
    let result = run_program_with_outcome(&mut executor, &mut program);
    (result, executor.get_output().to_string())
}

#[test]
fn test_quit_stops_the_run_and_reports_quit() {
    let (result, output) = run(&["10 PRINT \"BEFORE\"", "20 QUIT", "30 PRINT \"AFTER\""]);

    assert_eq!(result, Ok(RunOutcome::Quit));
    assert_eq!(output, "BEFORE\n");
}

#[test]
fn test_end_reports_ended() {
    let (result, output) = run(&["10 PRINT \"A\"", "20 END", "30 PRINT \"B\""]);

    assert_eq!(result, Ok(RunOutcome::Ended));
    assert_eq!(output, "A\n");
}

#[test]
fn test_program_without_end_reports_ended() {
    let (result, output) = run(&["10 PRINT \"A\""]);

    assert_eq!(result, Ok(RunOutcome::Ended));
    assert_eq!(output, "A\n");
}

#[test]
fn test_run_program_gives_ok_for_quit() {
    let mut executor = Executor::new();
    let mut program = ProgramStore::new();
    program.store_line(tokenize("10 QUIT").expect("line tokenizes"));

    assert_eq!(run_program(&mut executor, &mut program), Ok(()));
}

#[test]
fn test_quit_after_colon() {
    let (result, output) = run(&["10 PRINT \"A\": QUIT: PRINT \"B\"", "20 PRINT \"C\""]);

    assert_eq!(result, Ok(RunOutcome::Quit));
    assert_eq!(output, "A\n");
}

#[test]
fn test_quit_in_if_branch() {
    let (result, output) = run(&["10 IF 1 THEN QUIT", "20 PRINT \"AFTER\""]);

    assert_eq!(result, Ok(RunOutcome::Quit));
    assert_eq!(output, "");
}

#[test]
fn test_quit_in_procedure() {
    let (result, output) = run(&[
        "10 PROCleave",
        "20 PRINT \"AFTER\"",
        "30 END",
        "100 DEF PROCleave",
        "110 PRINT \"IN PROC\"",
        "120 QUIT",
        "130 ENDPROC",
    ]);

    assert_eq!(result, Ok(RunOutcome::Quit));
    assert_eq!(output, "IN PROC\n");
}

#[test]
fn test_quit_in_multi_line_function() {
    let (result, output) = run(&[
        "10 X = FNleave",
        "20 PRINT \"AFTER\"",
        "30 END",
        "100 DEF FNleave",
        "110 PRINT \"IN FN\"",
        "120 QUIT",
        "130 = 1",
    ]);

    assert_eq!(result, Ok(RunOutcome::Quit));
    assert_eq!(output, "IN FN\n");
}

#[test]
fn test_end_in_multi_line_function_reports_ended() {
    let (result, output) = run(&[
        "10 X = FNstop",
        "20 PRINT \"AFTER\"",
        "100 DEF FNstop",
        "110 END",
        "120 = 1",
    ]);

    assert_eq!(result, Ok(RunOutcome::Ended));
    assert_eq!(output, "");
}

/// Run the real binary with `input` on stdin. Fail if it does not stop in 5 seconds.
fn run_binary(input: &[u8]) -> (std::process::ExitStatus, String) {
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
        // The child can exit before it reads all of the input.
        let _ = stdin.write_all(input);
    }

    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait().expect("child status is readable") {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("interpreter did not stop within 5 seconds");
        }
        thread::sleep(Duration::from_millis(20));
    };

    let output = reader.join().expect("reader thread completes");
    (status, output)
}

#[test]
fn test_binary_leaves_after_quit_in_program() {
    let (status, output) = run_binary(
        b"10 PRINT \"BEFORE\"\n20 QUIT\n30 PRINT \"LINE30\"\nRUN\nPRINT \"AFTER\"\nEXIT\n",
    );

    assert!(status.success(), "exit status was {status}");
    assert!(output.contains("BEFORE"), "output was {output:?}");
    assert!(!output.contains("LINE30"), "output was {output:?}");
    assert!(!output.contains("AFTER"), "output was {output:?}");
    assert!(!output.contains("Error"), "output was {output:?}");
    assert!(output.contains("Goodbye!"), "output was {output:?}");
}
