//! Issue #85: LIST shows a hex number as typed, and puts no space between a
//! function keyword and its opening bracket.
//!
//! Before the correction, `&1F` listed as `31`, and `VAL("1")` listed as
//! `VAL ("1")`. RENUMBER also changed `GOTO &46` into a decimal line number.

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

const PROGRAM: [&str; 7] = [
    "10 A% = &1F + &FFFFFFFF",
    "20 PRINT VAL(\"&1F\");LEN(\"AB\");LEFT$(\"XYZ\",1);FNa(A%)",
    "30 PRINT (1 + 2);TAB(5);A%;~&FF",
    "40 GOTO &46",
    "50 PRINT \"NO\"",
    "70 END",
    "80 DEF FNa(X)= X * 2",
];

/// The output of RUN before the correction
const RUN_OUTPUT: &str = "312X60\n3    30FF\n";

#[test]
fn test_list_shows_hex_numbers_and_function_brackets_as_typed() {
    let input = format!("{}\nLIST\nEXIT\n", PROGRAM.join("\n"));
    let output = run_binary(&input);

    for line in PROGRAM {
        assert!(output.contains(line), "{line:?} not in {output:?}");
    }
}

#[test]
fn test_run_output_is_unchanged() {
    let (result, output) = run(&PROGRAM);

    assert_eq!(result, Ok(()));
    assert_eq!(output, RUN_OUTPUT);
}

#[test]
fn test_listed_lines_tokenize_to_the_same_tokens_and_run_the_same() {
    let listed: Vec<String> = PROGRAM
        .iter()
        .map(|source| {
            let line = tokenize(source).expect("line tokenizes");
            let listed = detokenize(&line).expect("line detokenizes");
            assert_eq!(tokenize(&listed).expect("listed line tokenizes"), line);
            listed
        })
        .collect();
    let source: Vec<&str> = listed.iter().map(String::as_str).collect();

    let (result, output) = run(&source);

    assert_eq!(result, Ok(()));
    assert_eq!(output, RUN_OUTPUT);
}

#[test]
fn test_renumber_does_not_change_a_hex_goto_target() {
    let input = format!("{}\nRENUMBER\nLIST\nEXIT\n", PROGRAM.join("\n"));
    let output = run_binary(&input);

    assert!(output.contains("40 GOTO &46"), "output was {output:?}");
    assert!(!output.contains("GOTO 60"), "output was {output:?}");
}
