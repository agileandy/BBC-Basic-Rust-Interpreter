//! Issue #86: a keyword matches only in upper case, as in BBC BASIC.
//!
//! Before the correction, the tokenizer changed each word to upper case
//! before the keyword lookup. So a variable such as `count` or `to` gave a
//! parse error. The typed commands of the command loop, such as `list` and
//! `run`, still work in any letter case.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;

/// Store the source lines, run the program and return the result and the output.
fn run(source: &[&str]) -> (Result<(), String>, String) {
    let mut executor = Executor::new();
    let mut program = ProgramStore::new();
    for line in source {
        program.store_line(tokenize(line).expect("line tokenizes"));
    }
    let result = run_program(&mut executor, &mut program);
    (result, executor.get_output().to_string())
}

/// Send the input to the real binary and return its standard output.
///
/// The binary has 5 seconds to stop.
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

    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(input.as_bytes())
        .expect("input is written");

    let deadline = Instant::now() + Duration::from_secs(5);
    while child
        .try_wait()
        .expect("child status is readable")
        .is_none()
    {
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
fn test_lower_case_count_is_a_variable() {
    let (result, output) = run(&["10 count = 5 : PRINT count"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "5\n");
}

#[test]
fn test_for_loop_with_lower_case_variable_runs() {
    let (result, output) = run(&["10 FOR i = 1 TO 3 : PRINT i : NEXT i"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n2\n3\n");
}

#[test]
fn test_lower_case_time_to_and_step_are_variables() {
    let (result, output) = run(&["10 time = 1 : to = 2 : step = 3 : PRINT time + to + step"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "6\n");
}

#[test]
fn test_lower_case_pi_val_len_and_endwhile_are_variables() {
    let (result, output) =
        run(&["10 pi = 1 : val = 2 : len = 3 : endwhile = 4 : PRINT pi + val + len + endwhile"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "10\n");
}

#[test]
fn test_lower_case_print_is_not_a_keyword() {
    let (result, output) = run(&["10 print 1"]);

    assert!(result.is_err(), "result was {:?}", result);
    assert_eq!(output, "");
}

#[test]
fn test_typed_commands_work_in_lower_case() {
    let output = run_binary("10 count = 5 : PRINT count\nlist\nrun\nEXIT\n");

    assert!(
        output.contains("> 10 count = 5:PRINT count\n"),
        "output was {:?}",
        output
    );
    assert!(output.contains("> 5\n"), "output was {:?}", output);
}
