//! Tests that each error shows its BBC BASIC text and its line, not an
//! internal name (issue #74).

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::{run_program, run_typed_line};
use bbc_basic_interpreter::tokenizer::tokenize;

/// Store the source lines, run the program and return the result and the
/// output. The run is in a thread with a time limit, so that a loop defect
/// fails the test and does not hang the test run.
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
        .expect("the program ran for more than 5 seconds")
}

/// Run the program and return its error text.
fn error_of(source: &[&str]) -> String {
    run(source).0.expect_err("the program stops with an error")
}

/// Run one typed line with no stored program and return its error text.
fn typed_error_of(line: &str) -> String {
    let mut executor = Executor::new();
    let program = ProgramStore::new();
    let line = tokenize(line).expect("typed line tokenizes");
    run_typed_line(&mut executor, &program, &line).expect_err("the typed line gives an error")
}

/// Run the binary with the given input lines and return its stdout.
/// Kill the binary and fail if it runs for more than 5 seconds.
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
    let reader = thread::spawn(move || {
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
        thread::sleep(Duration::from_millis(20));
    }
    reader.join().expect("reader thread ends")
}

// Runtime errors: "<BBC text> at line N"

#[test]
fn test_division_by_zero_text() {
    assert_eq!(
        error_of(&["10 X = 1", "20 PRINT X / 0"]),
        "Division by zero at line 20"
    );
}

#[test]
fn test_type_mismatch_text() {
    assert_eq!(
        error_of(&["10 PRINT \"A\" + 1"]),
        "Type mismatch at line 10"
    );
}

#[test]
fn test_subscript_text() {
    assert_eq!(
        error_of(&["10 DIM A(2)", "20 A(5) = 1"]),
        "Subscript at line 20"
    );
}

#[test]
fn test_string_too_long_text() {
    assert_eq!(
        error_of(&["10 A$ = STRING$(200, \"A\") + STRING$(100, \"B\")"]),
        "String too long at line 10"
    );
}

#[test]
fn test_too_big_text() {
    assert_eq!(error_of(&["10 A% = 3000000000"]), "Too big at line 10");
}

#[test]
fn test_on_range_text() {
    assert_eq!(
        error_of(&["10 ON 5 GOTO 20, 30", "20 END", "30 END"]),
        "ON range at line 10"
    );
}

#[test]
fn test_no_such_variable_text_has_the_name() {
    assert_eq!(
        error_of(&["10 PRINT Q%"]),
        "No such variable: Q% at line 10"
    );
}

#[test]
fn test_array_not_dimensioned_text_has_the_name() {
    assert_eq!(
        error_of(&[
            "10 PROCREAD",
            "20 END",
            "100 DEF PROCREAD",
            "110 LOCAL A%()",
            "120 PRINT A%(0)",
            "130 ENDPROC",
        ]),
        "Array not dimensioned: A% at line 120"
    );
}

#[test]
fn test_illegal_function_text() {
    assert_eq!(
        error_of(&["10 PRINT SQR(-1)"]),
        "Illegal function at line 10"
    );
}

#[test]
fn test_channel_text() {
    assert_eq!(error_of(&["10 PRINT BGET#99"]), "Channel at line 10");
}

#[test]
fn test_file_not_found_text_has_the_name() {
    assert_eq!(
        error_of(&["10 X = OPENIN(\"no-such-file-74.txt\")"]),
        "File not found: no-such-file-74.txt at line 10"
    );
}

#[test]
fn test_bad_call_text() {
    let path = format!("{}/issue_74_bad_call.txt", env!("CARGO_TARGET_TMPDIR"));
    let open = format!("10 X = OPENOUT(\"{}\")", path);
    assert_eq!(error_of(&[&open, "20 PRINT EOF#X"]), "Bad call at line 20");
}

#[test]
fn test_runtime_syntax_error_text() {
    assert_eq!(
        error_of(&["10 DEF FNa(X) = X", "20 PRINT FNb(1)"]),
        "Syntax error at line 20: Unknown function: b"
    );
}

// D4: errors with their own BBC text and ERR number

#[test]
fn test_out_of_data_text_and_err() {
    assert_eq!(error_of(&["10 READ A"]), "Out of DATA at line 10");
    let (_, output) = run(&[
        "10 ON ERROR GOTO 30",
        "20 READ A",
        "30 PRINT ERR; \" \"; ERL",
    ]);
    assert_eq!(output, "42 20\n");
}

#[test]
fn test_no_for_text_and_err() {
    assert_eq!(error_of(&["10 NEXT"]), "No FOR at line 10");
    let (_, output) = run(&["10 ON ERROR GOTO 30", "20 NEXT", "30 PRINT ERR"]);
    assert_eq!(output, "32\n");
}

#[test]
fn test_cant_match_for_text_and_err() {
    assert_eq!(
        error_of(&["10 FOR I = 1 TO 2", "20 NEXT J"]),
        "Can't match FOR at line 20"
    );
    let (_, output) = run(&[
        "10 ON ERROR GOTO 40",
        "20 FOR I = 1 TO 2",
        "30 NEXT J",
        "40 PRINT ERR",
    ]);
    assert_eq!(output, "33\n");
}

#[test]
fn test_not_local_text_and_err() {
    assert_eq!(error_of(&["10 LOCAL A"]), "Not LOCAL at line 10");
    let (_, output) = run(&["10 ON ERROR GOTO 30", "20 LOCAL A", "30 PRINT ERR"]);
    assert_eq!(output, "12\n");
}

#[test]
fn test_err_values_of_other_errors_do_not_change() {
    let (_, output) = run(&["10 ON ERROR GOTO 30", "20 PRINT 1 / 0", "30 PRINT ERR"]);
    assert_eq!(output, "18\n");
}

// Errors that stop the program

#[test]
fn test_no_such_line_text() {
    assert_eq!(error_of(&["10 GOTO 99"]), "No such line: 99 at line 10");
    assert_eq!(error_of(&["10 GOSUB 99"]), "No such line: 99 at line 10");
}

#[test]
fn test_no_gosub_text() {
    assert_eq!(error_of(&["10 RETURN"]), "No GOSUB at line 10");
}

#[test]
fn test_no_such_proc_text() {
    assert_eq!(
        error_of(&["10 PROCmissing"]),
        "No such FN/PROC: PROCmissing at line 10"
    );
}

#[test]
fn test_no_proc_text() {
    assert_eq!(error_of(&["10 ENDPROC"]), "No PROC at line 10");
}

#[test]
fn test_error_in_on_goto_expression_shows_its_text() {
    assert_eq!(
        error_of(&["10 ON 1 / 0 GOTO 20", "20 END"]),
        "Division by zero at line 10"
    );
}

#[test]
fn test_error_in_until_condition_shows_its_text() {
    assert_eq!(
        error_of(&["10 REPEAT", "20 UNTIL 1 / 0"]),
        "Division by zero at line 20"
    );
}

#[test]
fn test_procedure_argument_count_has_the_line() {
    assert_eq!(
        error_of(&["10 PROCa(1, 2)", "20 END", "30 DEF PROCa(X)", "40 ENDPROC"]),
        "Procedure a expects 1 parameters, got 2 at line 10"
    );
}

// Parse errors: "Syntax error at line N: <detail>"

#[test]
fn test_parse_error_text_shows_a_token_as_typed() {
    assert_eq!(
        error_of(&["10 PRINT )"]),
        "Syntax error at line 10: Unexpected token in expression: )"
    );
}

#[test]
fn test_unknown_statement_shows_the_keyword() {
    assert_eq!(
        error_of(&["10 TO"]),
        "Syntax error at line 10: Unknown statement: TO"
    );
}

#[test]
fn test_parse_error_message_of_invalid_assignment() {
    assert_eq!(
        error_of(&["10 FOO"]),
        "Syntax error at line 10: Invalid assignment"
    );
}

// Typed lines: the text with no line

#[test]
fn test_typed_line_runtime_error_has_no_line() {
    assert_eq!(typed_error_of("PRINT 1 / 0"), "Division by zero");
}

#[test]
fn test_typed_line_parse_error_has_no_line() {
    assert_eq!(
        typed_error_of("PRINT )"),
        "Syntax error: Unexpected token in expression: )"
    );
}

#[test]
fn test_typed_line_no_gosub_has_no_line() {
    assert_eq!(typed_error_of("RETURN"), "No GOSUB");
}

// The error text for ON ERROR handlers and REPORT

#[test]
fn test_report_prints_the_bbc_text_of_a_runtime_error() {
    let (_, output) = run(&["10 ON ERROR GOTO 30", "20 PRINT 1 / 0", "30 REPORT"]);
    assert_eq!(output, "Division by zero");
}

#[test]
fn test_report_prints_the_last_error_text() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 30",
        "20 PRINT Q%",
        "30 REPORT",
        "40 PRINT \" at line \"; ERL",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "No such variable: Q% at line 20\n");
}

#[test]
fn test_report_with_no_error_prints_nothing() {
    let (result, output) = run(&["10 REPORT", "20 PRINT \"X\""]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "X\n");
}

// The real binary

#[test]
fn test_binary_shows_runtime_error_text() {
    let output = run_binary("10 X = 0\n20 PRINT 1 / X\nRUN\nEXIT\n");
    assert!(
        output.contains("Error: Division by zero at line 20\n"),
        "output: {output}"
    );
    assert!(!output.contains("DivisionByZero"), "output: {output}");
}

#[test]
fn test_binary_shows_parse_error_text() {
    let output = run_binary("10 PRINT )\nRUN\nEXIT\n");
    assert!(
        output.contains("Error: Syntax error at line 10: Unexpected token in expression: )\n"),
        "output: {output}"
    );
}

#[test]
fn test_binary_shows_tokenizer_error_text() {
    let output = run_binary("10 PRINT \u{a3}\nEXIT\n");
    assert!(
        output.contains("Error: Syntax error at line 10: Unknown character '\u{a3}'\n"),
        "output: {output}"
    );
}
