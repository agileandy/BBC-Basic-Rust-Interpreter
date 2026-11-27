//! Tests that ON ERROR accepts statements: the handler is the rest of the
//! line after ON ERROR (issue #114).

use crate::common::run_with_limit as run;
use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::{run_program, run_typed_line, RunOutcome};
use bbc_basic_interpreter::tokenizer::tokenize;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// A change to the stored program between the run and the typed line.
enum Edit {
    /// Store this line (a new or a changed line)
    Store(&'static str),
    /// Delete the line with this number
    Delete(u16),
}

/// Store and run the program, apply the edit, then run the typed line. Give
/// the result of the typed line and all the output. Panic if this takes more
/// than 5 seconds.
fn run_edit_and_type(
    source: &'static [&'static str],
    edit: Edit,
    typed_line: &'static str,
) -> (Result<RunOutcome, String>, String) {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut executor = Executor::new();
        let mut program = ProgramStore::new();
        for line in source {
            program.store_line(tokenize(line).expect("line tokenizes"));
        }
        run_program(&mut executor, &mut program).expect("program runs");
        match edit {
            Edit::Store(line) => program.store_line(tokenize(line).expect("line tokenizes")),
            Edit::Delete(line_number) => program.delete_line(line_number),
        }
        let tokenized = tokenize(typed_line).expect("line tokenizes");
        let result = run_typed_line(&mut executor, &program, &tokenized);
        let _ = sender.send((result, executor.get_output().to_string()));
    });
    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("the typed line stops in 5 seconds")
}

/// Store the program, then run each typed line. Give the result of each
/// typed line and the output.
fn typed(source: &[&str], typed_lines: &[&str]) -> (Vec<Result<RunOutcome, String>>, String) {
    let mut executor = Executor::new();
    let mut program = ProgramStore::new();
    for line in source {
        program.store_line(tokenize(line).expect("line tokenizes"));
    }
    let results = typed_lines
        .iter()
        .map(|line| {
            let tokenized = tokenize(line).expect("line tokenizes");
            run_typed_line(&mut executor, &program, &tokenized)
        })
        .collect();
    (results, executor.get_output().to_string())
}

#[test]
fn test_handler_statements_run_when_an_error_occurs() {
    let (result, output) = run(&["10 ON ERROR PRINT \"caught\" : END", "20 PRINT 1/0"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "caught\n");
}

#[test]
fn test_handler_statements_read_err_and_erl() {
    let (result, output) = run(&[
        "10 ON ERROR PRINT ERR : PRINT ERL : END",
        "20 X = 1",
        "30 Y = X / 0",
    ]);

    assert_eq!(result, Ok(()));
    // 18 is the BBC BASIC error number for "Division by zero"
    assert_eq!(output, "        18\n        30\n");
}

// D2: in normal flow, ON ERROR stores the handler and the run continues at
// the next line
#[test]
fn test_handler_statements_do_not_run_when_no_error_occurs() {
    let (result, output) = run(&["10 ON ERROR PRINT \"caught\" : END", "20 PRINT \"done\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "done\n");
}

// D3: the handler stays set after an error
#[test]
fn test_handler_statements_catch_a_second_error() {
    let (result, output) = run(&[
        "10 N% = 0",
        "20 ON ERROR N% = N% + 1 : PRINT N% : IF N% < 2 THEN GOTO 40 ELSE END",
        "30 PRINT 1/0",
        "40 PRINT 2/0",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         1\n         2\n");
}

// D5: an IF in the handler line
#[test]
fn test_handler_with_if_checks_err() {
    let (result, output) = run(&[
        "10 ON ERROR IF ERR = 18 THEN PRINT \"div\" : END ELSE REPORT : END",
        "20 PRINT 1/0",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "div\n");
}

// The form of D5 in the issue. After the THEN part, the run continues at the
// line after the handler line.
#[test]
fn test_handler_with_if_continues_after_the_handler_line() {
    let (result, output) = run(&[
        "10 ON ERROR IF ERR = 18 THEN PRINT \"div\" ELSE REPORT : END",
        "20 N% = N% + 1",
        "30 IF N% = 1 THEN PRINT 1/0",
        "40 PRINT \"end\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "div\nend\n");
}

#[test]
fn test_handler_with_if_runs_else_part_for_other_error() {
    let (result, output) = run(&[
        "10 ON ERROR IF ERR = 18 THEN PRINT \"div\" : END ELSE REPORT : END",
        "20 GOTO 500",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "No such line: 500");
}

#[test]
fn test_handler_statements_run_outside_the_proc() {
    let (result, output) = run(&[
        "10 ON ERROR PRINT \"caught \"; A% : END",
        "20 A% = 1",
        "30 PROCfail",
        "40 END",
        "50 DEF PROCfail",
        "60 LOCAL A%",
        "70 A% = 2",
        "80 PRINT 1/0",
        "90 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "caught 1\n");
}

#[test]
fn test_on_error_goto_still_works() {
    let (result, output) = run(&["10 ON ERROR GOTO 100", "20 PRINT 1/0", "100 PRINT \"H\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "H\n");
}

#[test]
fn test_on_error_off_removes_statements_handler() {
    let (result, output) = run(&[
        "10 ON ERROR PRINT \"caught\" : END",
        "20 ON ERROR OFF",
        "30 PRINT 1/0",
    ]);

    assert_eq!(result, Err("Division by zero at line 30".to_string()));
    assert_eq!(output, "");
}

#[test]
fn test_later_on_error_goto_replaces_statements_handler() {
    let (result, output) = run(&[
        "10 ON ERROR PRINT \"first\" : END",
        "20 ON ERROR GOTO 100",
        "30 PRINT 1/0",
        "100 PRINT \"second\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "second\n");
}

// An error in the handler line itself stops the run with that error. It does
// not go to the handler again.
#[test]
fn test_error_in_handler_line_stops_the_run() {
    let (result, output) = run(&["10 ON ERROR PRINT \"caught\" : PRINT 1/0", "20 PRINT 2/0"]);

    assert_eq!(result, Err("Division by zero at line 10".to_string()));
    assert_eq!(output, "caught\n");
}

// D4: the typed line is not kept, so its statements cannot be a handler
#[test]
fn test_typed_on_error_statements_give_error() {
    let (results, output) = typed(
        &["10 PRINT 1/0"],
        &["ON ERROR PRINT \"caught\" : END", "GOTO 10"],
    );

    assert_eq!(
        results,
        vec![
            Err("ON ERROR with statements needs a line number".to_string()),
            Err("Division by zero at line 10".to_string()),
        ]
    );
    assert_eq!(output, "");
}

#[test]
fn test_typed_on_error_goto_still_works() {
    let (results, output) = typed(
        &["10 PRINT 1/0", "100 PRINT \"H\""],
        &["ON ERROR GOTO 100", "GOTO 10"],
    );

    assert_eq!(results, vec![Ok(RunOutcome::Ended), Ok(RunOutcome::Ended)]);
    assert_eq!(output, "H\n");
}

// K1: a handler is valid only while its line keeps the ON ERROR statement
const PROGRAM_WITH_STATEMENTS_HANDLER: [&str; 3] = [
    "10 ON ERROR PRINT \"caught\" : END",
    "20 PRINT 1/0",
    "30 END",
];

#[test]
fn test_changed_handler_line_removes_the_handler() {
    let (result, output) = run_edit_and_type(
        &PROGRAM_WITH_STATEMENTS_HANDLER,
        Edit::Store("10 A=1 : B=2 : PRINT \"stale ran\""),
        "GOTO 20",
    );

    assert_eq!(result, Err("Division by zero at line 20".to_string()));
    assert_eq!(output, "caught\n");
}

#[test]
fn test_deleted_handler_line_removes_the_handler() {
    let (result, output) = run_edit_and_type(
        &PROGRAM_WITH_STATEMENTS_HANDLER,
        Edit::Delete(10),
        "GOTO 20",
    );

    assert_eq!(result, Err("Division by zero at line 20".to_string()));
    assert_eq!(output, "caught\n");
}

// K2: the handler statements must be on one line
#[test]
fn test_block_if_in_handler_is_a_syntax_error() {
    let (result, output) = run(&[
        "10 ON ERROR IF ERR = 18 THEN",
        "20 PRINT \"div\"",
        "30 ENDIF",
        "40 PRINT \"ok\"",
    ]);

    assert_eq!(
        result,
        Err("Syntax error at line 10: Block IF cannot be in an ON ERROR handler".to_string())
    );
    assert_eq!(output, "");
}

#[test]
fn test_def_proc_in_handler_is_a_syntax_error() {
    let (result, _) = run(&["10 ON ERROR DEF PROCa", "20 ENDPROC"]);

    assert_eq!(
        result,
        Err("Syntax error at line 10: DEF cannot be in an ON ERROR handler".to_string())
    );
}

#[test]
fn test_def_fn_in_handler_is_a_syntax_error() {
    let (result, _) = run(&["10 ON ERROR DEF FNa = 1"]);

    assert_eq!(
        result,
        Err("Syntax error at line 10: DEF cannot be in an ON ERROR handler".to_string())
    );
}

// K3: ON ERROR needs a statement after it
#[test]
fn test_on_error_with_only_a_colon_is_a_syntax_error() {
    let (result, output) = run(&["10 ON ERROR :", "20 PRINT 1/0"]);

    assert_eq!(
        result,
        Err(
            "Syntax error at line 10: Expected GOTO, OFF or a statement after ON ERROR".to_string()
        )
    );
    assert_eq!(output, "");
}

#[test]
fn test_on_error_at_end_of_if_branch_is_a_syntax_error() {
    let (result, output) = run(&["10 IF TRUE THEN ON ERROR", "20 PRINT 1/0"]);

    assert_eq!(
        result,
        Err(
            "Syntax error at line 10: Expected GOTO, OFF or a statement after ON ERROR".to_string()
        )
    );
    assert_eq!(output, "");
}

#[test]
fn test_on_error_local_is_a_syntax_error() {
    let (result, _) = run(&["10 ON ERROR LOCAL PRINT \"caught\""]);

    assert_eq!(
        result,
        Err("Syntax error at line 10: ON ERROR LOCAL is not supported".to_string())
    );
}
