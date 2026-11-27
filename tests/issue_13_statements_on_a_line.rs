//! Issue 13: all the statements on a line run, in order.
//!
//! A colon separates statements on one line. Control flow (loops, jumps,
//! calls and IF) works across the statements of one line.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;

/// Store the source lines, run the program and return the result and the output.
///
/// The run has a time limit, so that a loop defect fails the test and does
/// not stop the test run.
fn run(source: &[&str]) -> (Result<(), String>, String) {
    let source: Vec<String> = source.iter().map(|line| line.to_string()).collect();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut executor = Executor::new();
        let mut program = ProgramStore::new();
        for line in &source {
            program.store_line(tokenize(line).expect("line tokenizes"));
        }
        let result = run_program(&mut executor, &mut program);
        let _ = sender.send((result, executor.get_output().to_string()));
    });
    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("the program stops in 5 seconds")
}

#[test]
fn test_two_assignments_on_one_line() {
    let (result, output) = run(&["10 A% = 1 : B% = 2", "20 PRINT A%; B%"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "12\n");
}

#[test]
fn test_three_statements_on_one_line() {
    let (result, output) = run(&["10 A% = 1 : B% = A% + 1 : PRINT A% + B%"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "3\n");
}

#[test]
fn test_for_loop_on_one_line() {
    let (result, output) = run(&["10 FOR I% = 1 TO 3 : PRINT I% : NEXT", "20 PRINT \"DONE\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n2\n3\nDONE\n");
}

#[test]
fn test_for_loop_with_statements_after_for_and_next_on_later_line() {
    let (result, output) = run(&[
        "10 FOR I% = 1 TO 2 : PRINT \"A\"",
        "20 PRINT \"B\"",
        "30 NEXT I% : PRINT \"END\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "A\nB\nA\nB\nEND\n");
}

#[test]
fn test_repeat_loop_on_one_line() {
    let (result, output) = run(&[
        "10 X% = 0",
        "20 REPEAT : X% = X% + 1 : UNTIL X% = 3",
        "30 PRINT X%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "3\n");
}

#[test]
fn test_while_loop_on_one_line_with_true_condition() {
    let (result, output) = run(&[
        "10 X% = 0",
        "20 WHILE X% < 3 : X% = X% + 1 : ENDWHILE : PRINT X%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "3\n");
}

#[test]
fn test_while_loop_on_one_line_with_false_condition() {
    let (result, output) = run(&[
        "10 X% = 5",
        "20 WHILE X% < 3 : PRINT \"BODY\" : ENDWHILE : PRINT \"AFTER\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "AFTER\n");
}

#[test]
fn test_if_false_runs_no_then_statement() {
    let (result, output) = run(&[
        "10 A% = 0 : B% = 0 : X% = 0",
        "20 IF X% THEN A% = 1 : B% = 2",
        "30 PRINT A%; B%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "00\n");
}

#[test]
fn test_if_true_runs_all_then_statements() {
    let (result, output) = run(&[
        "10 X% = 1",
        "20 IF X% THEN A% = 1 : B% = 2",
        "30 PRINT A%; B%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "12\n");
}

#[test]
fn test_if_else_runs_the_correct_pair() {
    let (result, output) = run(&[
        "10 X% = 0",
        "20 IF X% THEN A% = 1 : B% = 2 ELSE A% = 3 : B% = 4",
        "30 PRINT A%; B%",
        "40 X% = 1",
        "50 IF X% THEN C% = 1 : D% = 2 ELSE C% = 3 : D% = 4",
        "60 PRINT C%; D%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "34\n12\n");
}

#[test]
fn test_gosub_returns_to_statement_after_call() {
    let (result, output) = run(&[
        "10 GOSUB 100 : PRINT \"BACK\"",
        "20 END",
        "100 PRINT \"SUB\"",
        "110 RETURN",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "SUB\nBACK\n");
}

#[test]
fn test_two_procedure_calls_on_one_line() {
    let (result, output) = run(&[
        "10 PROCa : PROCb",
        "20 END",
        "100 DEF PROCa",
        "110 PRINT \"A\"",
        "120 ENDPROC",
        "200 DEF PROCb : PRINT \"B\" : ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "A\nB\n");
}

#[test]
fn test_two_procedure_calls_in_then_branch() {
    let (result, output) = run(&[
        "10 IF 1 THEN PROCa : PROCb : PRINT \"C\"",
        "20 END",
        "100 DEF PROCa : PRINT \"A\" : ENDPROC",
        "200 DEF PROCb : PRINT \"B\" : ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "A\nB\nC\n");
}

#[test]
fn test_goto_in_middle_of_line_skips_rest_of_line() {
    let (result, output) = run(&[
        "10 PRINT \"A\" : GOTO 30 : PRINT \"SKIPPED\"",
        "20 PRINT \"SKIPPED\"",
        "30 PRINT \"TARGET\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "A\nTARGET\n");
}

#[test]
fn test_colon_in_string_is_not_a_separator() {
    let (result, output) = run(&["10 PRINT \"A:B\" : PRINT \"C\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "A:B\nC\n");
}

#[test]
fn test_colon_after_rem_is_part_of_comment() {
    let (result, output) = run(&[
        "10 PRINT \"A\" : REM note : PRINT \"NOT RUN\"",
        "20 PRINT \"B\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "A\nB\n");
}

#[test]
fn test_data_after_statement_and_colon_in_data_string() {
    let (result, output) = run(&[
        "10 READ A$, B% : PRINT A$; B%",
        "20 X% = 1 : DATA \"P:Q\", 7",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "P:Q7\n");
}

#[test]
fn test_multiline_function_with_if_return_and_two_statement_line() {
    let (result, output) = run(&[
        "10 PRINT FNfact(4)",
        "20 END",
        "100 DEF FNfact(N%)",
        "110 IF N% <= 1 THEN =1",
        "120 R% = N% : R% = R% * FNfact(N% - 1)",
        "130 =R%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "24\n");
}

#[test]
fn test_error_in_third_statement_gives_line_in_erl() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 A% = 1 : B% = 0 : C% = A% DIV B%",
        "30 END",
        "100 PRINT \"ERL \"; ERL",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "ERL 20\n");
}

/// Give the input lines to the interpreter binary and return its output.
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
fn test_immediate_mode_runs_each_statement() {
    let output = run_binary("A% = 41 : B% = A% + 1 : PRINT B%\nEXIT\n");

    assert!(output.contains("42"), "output was {output:?}");
}
