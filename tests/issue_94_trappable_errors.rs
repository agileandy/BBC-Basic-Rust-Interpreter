//! Tests that each error that BBC BASIC can trap goes to the ON ERROR
//! handler with the BBC error number, and that the text with no handler
//! does not change (issue #94).

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
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

/// The handler line of each trapped program: it prints ERR and ERL.
const HANDLER: &str = "100 PRINT ERR; \" \"; ERL";

/// Run `body` with an ON ERROR handler at line 100 and return the output.
fn trapped_output(body: &[&str]) -> String {
    let mut source = vec!["10 ON ERROR GOTO 100"];
    source.extend_from_slice(body);
    source.push("90 END");
    source.push(HANDLER);
    let (result, output) = run(&source);
    assert_eq!(result, Ok(()), "the handler runs and the program ends");
    output
}

// D1: No such line, ERR 41

#[test]
fn test_goto_missing_line_is_trapped() {
    assert_eq!(trapped_output(&["20 GOTO 500"]), "41 20\n");
    assert_eq!(error_of(&["20 GOTO 500"]), "No such line: 500 at line 20");
}

#[test]
fn test_gosub_missing_line_is_trapped() {
    assert_eq!(trapped_output(&["20 GOSUB 500"]), "41 20\n");
    assert_eq!(error_of(&["20 GOSUB 500"]), "No such line: 500 at line 20");
}

#[test]
fn test_calculated_goto_missing_line_is_trapped() {
    assert_eq!(trapped_output(&["20 X% = 500", "30 GOTO X%"]), "41 30\n");
    assert_eq!(
        error_of(&["20 X% = 500", "30 GOTO X%"]),
        "No such line: 500 at line 30"
    );
}

#[test]
fn test_calculated_gosub_missing_line_is_trapped() {
    assert_eq!(trapped_output(&["20 X% = 500", "30 GOSUB X%"]), "41 30\n");
    assert_eq!(
        error_of(&["20 X% = 500", "30 GOSUB X%"]),
        "No such line: 500 at line 30"
    );
}

#[test]
fn test_on_goto_missing_line_is_trapped() {
    assert_eq!(trapped_output(&["20 ON 1 GOTO 500"]), "41 20\n");
    assert_eq!(
        error_of(&["20 ON 1 GOTO 500"]),
        "No such line: 500 at line 20"
    );
}

#[test]
fn test_on_gosub_missing_line_is_trapped() {
    assert_eq!(trapped_output(&["20 ON 1 GOSUB 500"]), "41 20\n");
    assert_eq!(
        error_of(&["20 ON 1 GOSUB 500"]),
        "No such line: 500 at line 20"
    );
}

// D1: No GOSUB, ERR 38, and No PROC, ERR 13

#[test]
fn test_return_with_no_gosub_is_trapped() {
    assert_eq!(trapped_output(&["20 RETURN"]), "38 20\n");
    assert_eq!(error_of(&["20 RETURN"]), "No GOSUB at line 20");
}

#[test]
fn test_endproc_with_no_proc_is_trapped() {
    assert_eq!(trapped_output(&["20 ENDPROC"]), "13 20\n");
    assert_eq!(error_of(&["20 ENDPROC"]), "No PROC at line 20");
}

// D1 and D5: No such FN/PROC, ERR 29

#[test]
fn test_unknown_proc_is_trapped() {
    assert_eq!(trapped_output(&["20 PROCz"]), "29 20\n");
    assert_eq!(error_of(&["20 PROCz"]), "No such FN/PROC: PROCz at line 20");
}

#[test]
fn test_unknown_fn_is_trapped() {
    assert_eq!(trapped_output(&["20 PRINT FNq"]), "29 20\n");
    assert_eq!(
        error_of(&["20 PRINT FNq"]),
        "No such FN/PROC: FNq at line 20"
    );
}

#[test]
fn test_unknown_fn_with_arguments_in_each_type() {
    assert_eq!(
        error_of(&["20 X% = FNq(1)"]),
        "No such FN/PROC: FNq at line 20"
    );
    assert_eq!(
        error_of(&["20 X = FNq(1)"]),
        "No such FN/PROC: FNq at line 20"
    );
    assert_eq!(
        error_of(&["20 A$ = FNq$(1)"]),
        "No such FN/PROC: FNq$ at line 20"
    );
}

#[test]
fn test_defined_fn_still_runs() {
    let (result, output) = run(&[
        "10 PRINT FNdouble(4)",
        "20 END",
        "30 DEF FNdouble(X) = X * 2",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "8\n");
}

#[test]
fn test_unknown_built_in_name_keeps_syntax_error() {
    assert_eq!(
        error_of(&["20 X = TIME(1)"]),
        "Syntax error at line 20: Unknown function: TIME"
    );
}

#[test]
fn test_undefined_fn_with_built_in_name_does_not_call_the_built_in() {
    assert_eq!(trapped_output(&["20 PRINT FNSIN(0)"]), "29 20\n");
    assert_eq!(
        error_of(&["20 PRINT FNSIN(0)"]),
        "No such FN/PROC: FNSIN at line 20"
    );
}

// D1: an error in a WHILE, ENDWHILE, ON or PROC argument expression

#[test]
fn test_error_in_until_condition_is_trapped() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 REPEAT",
        "30 UNTIL 1/0",
        "40 END",
        "100 PRINT \"caught \"; ERR; \" \"; ERL",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "caught 18 30\n");
    assert_eq!(
        error_of(&["20 REPEAT", "30 UNTIL 1/0"]),
        "Division by zero at line 30"
    );
}

#[test]
fn test_error_in_fn_body_called_by_until_gives_body_line() {
    let body = [
        "20 REPEAT",
        "30 UNTIL FNa",
        "40 END",
        "50 DEF FNa",
        "60 = 1 / 0",
    ];
    assert_eq!(trapped_output(&body), "18 60\n");
    assert_eq!(error_of(&body), "Division by zero at line 60");
}

#[test]
fn test_error_in_while_condition_is_trapped() {
    assert_eq!(
        trapped_output(&["20 WHILE 1 / 0", "30 ENDWHILE"]),
        "18 20\n"
    );
    assert_eq!(
        error_of(&["20 WHILE 1 / 0", "30 ENDWHILE"]),
        "Division by zero at line 20"
    );
}

#[test]
fn test_error_in_endwhile_condition_is_trapped() {
    let body = ["20 A = 1", "30 WHILE 1 / A", "40 A = 0", "50 ENDWHILE"];
    assert_eq!(trapped_output(&body), "18 50\n");
    assert_eq!(error_of(&body), "Division by zero at line 50");
}

#[test]
fn test_error_in_on_goto_expression_is_trapped() {
    assert_eq!(
        trapped_output(&["20 ON 1 / 0 GOTO 30", "30 END"]),
        "18 20\n"
    );
    assert_eq!(
        error_of(&["20 ON 1 / 0 GOTO 30", "30 END"]),
        "Division by zero at line 20"
    );
}

#[test]
fn test_error_in_on_gosub_expression_is_trapped() {
    assert_eq!(
        trapped_output(&["20 ON 1 / 0 GOSUB 30", "30 END"]),
        "18 20\n"
    );
    assert_eq!(
        error_of(&["20 ON 1 / 0 GOSUB 30", "30 END"]),
        "Division by zero at line 20"
    );
}

#[test]
fn test_error_in_proc_argument_is_trapped() {
    let body = ["20 PROCa(1 / 0)", "30 END", "40 DEF PROCa(X)", "50 ENDPROC"];
    assert_eq!(trapped_output(&body), "18 20\n");
    assert_eq!(error_of(&body), "Division by zero at line 20");
}

// D4: ERR values of other errors

#[test]
fn test_channel_err_is_222() {
    assert_eq!(trapped_output(&["20 PRINT EOF#99"]), "222 20\n");
}

#[test]
fn test_bad_call_err_is_30() {
    let path = format!("{}/issue_94_bad_call.txt", env!("CARGO_TARGET_TMPDIR"));
    let open = format!("20 X = OPENOUT(\"{}\")", path);
    assert_eq!(trapped_output(&[&open, "30 PRINT EOF#X"]), "30 30\n");
}

#[test]
fn test_syntax_error_err_is_16() {
    assert_eq!(trapped_output(&["20 X = TIME(1)"]), "16 20\n");
}

// D7: the handler of an error in a PROC sees the outer values

#[test]
fn test_handler_after_error_in_proc_sees_outer_values() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 Y = 5",
        "30 PROCa",
        "40 END",
        "50 DEF PROCa",
        "60 LOCAL Y",
        "70 Y = 9",
        "80 PRINT 1 / 0",
        "90 ENDPROC",
        "100 PRINT ERR; \" \"; ERL; \" \"; Y",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "18 80 5\n");
}

#[test]
fn test_handler_after_error_in_fn_sees_outer_values() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 Y = 5",
        "30 PRINT FNa",
        "40 END",
        "50 DEF FNa",
        "60 LOCAL Y",
        "70 Y = 9",
        "80 = 1 / 0",
        "100 PRINT ERR; \" \"; ERL; \" \"; Y",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "18 80 5\n");
}

#[test]
fn test_handler_can_call_a_proc_after_error_in_proc() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 PROCa",
        "30 END",
        "50 DEF PROCa",
        "60 PRINT 1 / 0",
        "70 ENDPROC",
        "80 DEF PROCb",
        "85 PRINT \"b\"",
        "90 ENDPROC",
        "100 ON ERROR OFF",
        "110 PROCb",
        "120 PRINT \"done\"",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "b\ndone\n");
}
