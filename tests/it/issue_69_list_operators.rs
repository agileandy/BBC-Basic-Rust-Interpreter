//! Issue #69: LIST shows the operators `<=`, `>=` and `<>` as typed.
//!
//! Before the correction, LIST put a space between the 2 characters, for
//! example `< =`. The listed line must also tokenize to the same tokens.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::{detokenize, tokenize};

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

const PROGRAM: [&str; 3] = [
    "10 IF A% <= B% THEN PRINT \"LE\"",
    "20 IF A% >= B% THEN PRINT \"GE\"",
    "30 IF A% <> B% THEN PRINT \"NE\"",
];

#[test]
fn test_list_shows_comparison_operators_as_typed() {
    let input = format!("{}\nLIST\nEXIT\n", PROGRAM.join("\n"));
    let output = run_binary(&input);

    for line in PROGRAM {
        assert!(output.contains(line), "output was {output:?}");
    }
    for split in ["< =", "> =", "< >"] {
        assert!(!output.contains(split), "output was {output:?}");
    }
}

#[test]
fn test_listed_lines_tokenize_to_the_same_tokens() {
    for source in PROGRAM {
        let line = tokenize(source).expect("line tokenizes");
        let listed = detokenize(&line).expect("line detokenizes");
        assert_eq!(listed, source);
        assert_eq!(tokenize(&listed).expect("listed line tokenizes"), line);
    }
}

#[test]
fn test_listed_lines_run_as_typed() {
    let listed: Vec<String> = PROGRAM
        .iter()
        .map(|source| detokenize(&tokenize(source).expect("line tokenizes")).expect("detokenizes"))
        .collect();
    let mut source = vec!["5 A% = 1 : B% = 2"];
    source.extend(listed.iter().map(String::as_str));

    let (result, output) = run(&source);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "LE\nNE\n");
}
