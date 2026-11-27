//! Helpers that 3 or more test modules shared as identical copies.
//!
//! Each helper has the body of the copies that it replaces. A module that had a
//! copy imports the helper with its old name, for example
//! `use crate::common::run_lines as run;`, so that each test is unchanged.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;

/// The time limit of `run_in_dir`.
const TIME_LIMIT: Duration = Duration::from_secs(5);

/// Store the lines in the library, run the program, and give the result and
/// the output. There is no time limit.
pub fn run_lines(source: &[&str]) -> (Result<(), String>, String) {
    let mut executor = Executor::new();
    let mut program = ProgramStore::new();
    for line in source {
        program.store_line(tokenize(line).expect("line tokenizes"));
    }
    let result = run_program(&mut executor, &mut program);
    (result, executor.get_output().to_string())
}

/// Do `run_lines` on a thread with a time limit of 5 seconds. Panic with the
/// message "program ends within 5 seconds and does not panic" at the limit.
pub fn run_with_limit(source: &[&str]) -> (Result<(), String>, String) {
    let source: Vec<String> = source.iter().map(|line| line.to_string()).collect();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let lines: Vec<&str> = source.iter().map(String::as_str).collect();
        let _ = sender.send(run_lines(&lines));
    });
    receiver
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("program ends within 5 seconds and does not panic")
}

/// Store the lines and run the program on a thread. Panic with `message` if
/// the program does not stop in 5 seconds.
fn run_timed(source: &[&str], message: &str) -> (Result<(), String>, String) {
    let lines: Vec<String> = source.iter().map(|line| line.to_string()).collect();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut executor = Executor::new();
        let mut program = ProgramStore::new();
        for line in &lines {
            program.store_line(tokenize(line).expect("line tokenizes"));
        }
        let result = run_program(&mut executor, &mut program);
        let _ = sender.send((result, executor.get_output().to_string()));
    });
    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect(message)
}

/// `run_timed` with the message "program stops in 5 seconds".
pub fn run_timed_stops(source: &[&str]) -> (Result<(), String>, String) {
    run_timed(source, "program stops in 5 seconds")
}

/// `run_timed` with the message "the program stops in 5 seconds".
pub fn run_timed_the_program_stops(source: &[&str]) -> (Result<(), String>, String) {
    run_timed(source, "the program stops in 5 seconds")
}

/// `run_timed` with the message "the program ran for more than 5 seconds".
pub fn run_timed_ran_for(source: &[&str]) -> (Result<(), String>, String) {
    run_timed(source, "the program ran for more than 5 seconds")
}

/// Queue the replies for INPUT, store the lines, run the program, and give the
/// result and the output. There is no time limit.
pub fn run_with_replies(source: &[&str], replies: &[&str]) -> (Result<(), String>, String) {
    let mut executor = Executor::new();
    for reply in replies {
        executor.queue_input(reply);
    }
    let mut program = ProgramStore::new();
    for line in source {
        program.store_line(tokenize(line).expect("line tokenizes"));
    }
    let result = run_program(&mut executor, &mut program);
    (result, executor.get_output().to_string())
}

/// Run the binary in `dir`, write each command as a line, and give the exit
/// status and stdout. Kill the binary after `TIME_LIMIT`.
pub fn run_in_dir(dir: &Path, commands: &[&str]) -> (std::process::ExitStatus, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_bbc-basic-interpreter"))
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("interpreter starts");

    {
        let mut stdin = child.stdin.take().expect("stdin is piped");
        for command in commands {
            writeln!(stdin, "{}", command).expect("command is written");
        }
    }

    let deadline = Instant::now() + TIME_LIMIT;
    let status = loop {
        if let Some(status) = child.try_wait().expect("child status is read") {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().ok();
            child.wait().ok();
            panic!("interpreter did not end within {:?}", TIME_LIMIT);
        }
        std::thread::sleep(Duration::from_millis(20));
    };

    let output = child.wait_with_output().expect("output is read");
    (status, String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Run the binary, start a thread that reads stdout, then write the input.
/// Kill the binary after 5 seconds.
pub fn run_binary_read_first(input: &str) -> String {
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

/// Run the binary, write the input, then start a thread that reads stdout.
/// Kill the binary after 5 seconds.
pub fn run_binary_write_first(input: &str) -> String {
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
