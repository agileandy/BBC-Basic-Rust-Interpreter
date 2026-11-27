//! Tests that a line typed with no line number uses the control flow of a
//! program line (issue #53).

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::{run_program_with_outcome, run_typed_line, RunOutcome};
use bbc_basic_interpreter::tokenizer::tokenize;

/// Store the program lines, then run each typed line in turn. Return the
/// result of each typed line and the output.
///
/// The run is in a thread with a time limit, so a loop defect fails the
/// test and does not hang the test run.
fn typed(
    program_lines: &[&str],
    typed_lines: &[&str],
) -> (Vec<Result<RunOutcome, String>>, String) {
    let program_lines: Vec<String> = program_lines.iter().map(|l| l.to_string()).collect();
    let typed_lines: Vec<String> = typed_lines.iter().map(|l| l.to_string()).collect();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut executor = Executor::new();
        let mut program = ProgramStore::new();
        for line in &program_lines {
            program.store_line(tokenize(line).expect("line tokenizes"));
        }
        let results = typed_lines
            .iter()
            .map(|line| {
                let line = tokenize(line).expect("typed line tokenizes");
                run_typed_line(&mut executor, &program, &line)
            })
            .collect();
        let _ = sender.send((results, executor.get_output().to_string()));
    });
    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("typed lines stop in 5 seconds")
}

#[test]
fn test_typed_for_loop_runs_each_value() {
    let (results, output) = typed(&[], &["FOR I%=1 TO 3 : PRINT I% : NEXT"]);

    assert_eq!(results, vec![Ok(RunOutcome::Ended)]);
    assert_eq!(output, "1\n2\n3\n");
}

#[test]
fn test_typed_repeat_loop_runs_until_condition() {
    let (results, output) = typed(&[], &["I%=0 : REPEAT : I%=I%+1 : PRINT I% : UNTIL I%=3"]);

    assert_eq!(results, vec![Ok(RunOutcome::Ended)]);
    assert_eq!(output, "1\n2\n3\n");
}

#[test]
fn test_typed_while_loop_runs_while_condition() {
    let (results, output) = typed(
        &[],
        &["I%=0 : WHILE I%<3 : I%=I%+1 : PRINT I% : ENDWHILE : PRINT \"end\""],
    );

    assert_eq!(results, vec![Ok(RunOutcome::Ended)]);
    assert_eq!(output, "1\n2\n3\nend\n");
}

#[test]
fn test_typed_nested_if_uses_next_else_rule() {
    let cases = [
        ((0, 0), "z\n"),
        ((0, 1), "z\n"),
        ((1, 0), "z\n"),
        ((1, 1), "y\n"),
    ];
    for ((a, b), expected) in cases {
        let values = format!("A%={} : B%={}", a, b);
        let (results, output) = typed(
            &[],
            &[
                &values,
                "IF A% THEN IF B% THEN PRINT \"y\" ELSE PRINT \"z\"",
            ],
        );

        assert_eq!(results, vec![Ok(RunOutcome::Ended); 2], "A%={} B%={}", a, b);
        assert_eq!(output, expected, "A%={} B%={}", a, b);
    }
}

#[test]
fn test_typed_gosub_runs_stored_lines_and_returns() {
    let (results, output) = typed(
        &["10 PRINT \"SUB\"", "20 RETURN"],
        &["GOSUB 10 : PRINT \"B\""],
    );

    assert_eq!(results, vec![Ok(RunOutcome::Ended)]);
    assert_eq!(output, "SUB\nB\n");
}

#[test]
fn test_typed_proc_call_runs_stored_procedure_and_returns() {
    let (results, output) = typed(
        &[
            "10 END",
            "100 DEF PROCshow(N%)",
            "110 PRINT N%*2",
            "120 ENDPROC",
        ],
        &["PROCshow(4) : PRINT \"B\""],
    );

    assert_eq!(results, vec![Ok(RunOutcome::Ended)]);
    assert_eq!(output, "8\nB\n");
}

#[test]
fn test_typed_line_calls_stored_single_line_function() {
    let (results, output) = typed(&["10 END", "100 DEF FNsq(X)=X*X"], &["PRINT FNsq(3)"]);

    assert_eq!(results, vec![Ok(RunOutcome::Ended)]);
    assert_eq!(output, "9\n");
}

#[test]
fn test_typed_line_calls_stored_multi_line_function() {
    let (results, output) = typed(
        &["10 END", "100 DEF FNtwice(X)", "110 Y=X*2", "120 =Y"],
        &["PRINT FNtwice(5)"],
    );

    assert_eq!(results, vec![Ok(RunOutcome::Ended)]);
    assert_eq!(output, "10\n");
}

#[test]
fn test_typed_function_definition_stays_with_stored_program() {
    let (results, output) = typed(
        &["10 PRINT \"PROGRAM\""],
        &["DEF FNdbl(X)=X*2", "PRINT FNdbl(4)"],
    );

    assert_eq!(results, vec![Ok(RunOutcome::Ended); 2]);
    assert_eq!(output, "8\n");
}

#[test]
fn test_typed_variables_stay_between_lines() {
    let (results, output) = typed(&["10 PRINT \"PROGRAM\""], &["A%=5", "PRINT A%"]);

    assert_eq!(results, vec![Ok(RunOutcome::Ended); 2]);
    assert_eq!(output, "5\n");
}

#[test]
fn test_typed_goto_runs_stored_program_and_does_not_return() {
    let (results, output) = typed(
        &["10 PRINT \"A\"", "20 PRINT \"B\""],
        &["GOTO 20 : PRINT \"NO\""],
    );

    assert_eq!(results, vec![Ok(RunOutcome::Ended)]);
    assert_eq!(output, "B\n");
}

#[test]
fn test_second_typed_next_after_complete_loop_gives_error() {
    let (results, output) = typed(&[], &["FOR I%=1 TO 2 : NEXT", "NEXT"]);

    assert_eq!(results[0], Ok(RunOutcome::Ended));
    assert!(results[1].is_err(), "results were {results:?}");
    assert_eq!(output, "");
}

#[test]
fn test_error_in_typed_loop_leaves_no_open_loop() {
    let (results, output) = typed(&[], &["FOR I%=1 TO 3 : PRINT 1/0 : NEXT", "NEXT"]);

    assert_eq!(results[0], Err("Division by zero".to_string()));
    assert!(results[1].is_err(), "results were {results:?}");
    assert_eq!(output, "");
}

#[test]
fn test_error_in_stored_subroutine_leaves_no_return_entry() {
    let (results, _) = typed(
        &["10 PRINT 1/0", "20 RETURN"],
        &["GOSUB 10 : PRINT \"B\"", "RETURN"],
    );

    assert_eq!(results[0], Err("Division by zero at line 10".to_string()));
    assert!(results[1].is_err(), "results were {results:?}");
}

#[test]
fn test_typed_end_ends_the_line() {
    let (results, output) = typed(&[], &["PRINT \"A\" : END : PRINT \"B\""]);

    assert_eq!(results, vec![Ok(RunOutcome::Ended)]);
    assert_eq!(output, "A\n");
}

#[test]
fn test_typed_quit_reports_quit() {
    let (results, output) = typed(&[], &["PRINT \"A\" : QUIT : PRINT \"B\""]);

    assert_eq!(results, vec![Ok(RunOutcome::Quit)]);
    assert_eq!(output, "A\n");
}

#[test]
fn test_deleted_stored_definitions_are_not_callable() {
    let mut executor = Executor::new();
    let mut program = ProgramStore::new();
    for line in [
        "100 DEF PROCa",
        "110 PRINT \"PROC\"",
        "120 ENDPROC",
        "200 DEF FNb",
        "210 =7",
    ] {
        program.store_line(tokenize(line).expect("line tokenizes"));
    }
    let call = tokenize("PROCa : PRINT FNb").expect("line tokenizes");
    assert_eq!(
        run_typed_line(&mut executor, &program, &call),
        Ok(RunOutcome::Ended)
    );
    assert_eq!(executor.get_output(), "PROC\n7\n");

    for line in [100, 110, 120, 200, 210] {
        program.delete_line(line);
    }
    let proc_call = tokenize("PROCa").expect("line tokenizes");
    let fn_call = tokenize("PRINT FNb").expect("line tokenizes");

    assert!(run_typed_line(&mut executor, &program, &proc_call).is_err());
    assert!(run_typed_line(&mut executor, &program, &fn_call).is_err());
}

#[test]
fn test_stored_line_that_does_not_parse_does_not_stop_typed_line() {
    let (results, output) = typed(
        &["10 PRINT \"SUB\"", "20 RETURN", "30 FOR"],
        &["GOSUB 10 : PRINT \"B\""],
    );

    assert_eq!(results, vec![Ok(RunOutcome::Ended)]);
    assert_eq!(output, "SUB\nB\n");
}

/// Run `session` in a thread with a time limit, so that a loop defect fails
/// the test and does not hang the test run.
fn with_limit<T: Send + 'static>(session: impl FnOnce() -> T + Send + 'static) -> T {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(session());
    });
    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("session stops in 5 seconds")
}

/// Store the source lines in `program`.
fn store(program: &mut ProgramStore, source: &[&str]) {
    for line in source {
        program.store_line(tokenize(line).expect("line tokenizes"));
    }
}

/// Run one typed line.
fn type_line(
    executor: &mut Executor,
    program: &ProgramStore,
    line: &str,
) -> Result<RunOutcome, String> {
    let line = tokenize(line).expect("typed line tokenizes");
    run_typed_line(executor, program, &line)
}

const PROC_WITH_LOCAL_ERROR: [&str; 4] = [
    "100 DEF PROCp",
    "110 LOCAL X",
    "120 X=5:PRINT 1/0",
    "130 ENDPROC",
];

#[test]
fn test_error_in_typed_proc_call_restores_local_variable() {
    let (results, output) = typed(&PROC_WITH_LOCAL_ERROR, &["X=1", "PROCp", "PRINT X"]);

    assert_eq!(results[1], Err("Division by zero at line 120".to_string()));
    assert_eq!(output, "1\n");
}

#[test]
fn test_end_in_typed_proc_call_restores_local_variable() {
    let (results, output) = typed(
        &["100 DEF PROCp", "110 LOCAL X", "120 X=5:END", "130 ENDPROC"],
        &["X=1", "PROCp", "PRINT X"],
    );

    assert_eq!(results, vec![Ok(RunOutcome::Ended); 3]);
    assert_eq!(output, "1\n");
}

#[test]
fn test_error_in_run_restores_local_variable() {
    let output = with_limit(|| {
        let mut executor = Executor::new();
        let mut program = ProgramStore::new();
        store(&mut program, &["10 X=1", "20 PROCp", "30 END"]);
        store(&mut program, &PROC_WITH_LOCAL_ERROR);
        let result = run_program_with_outcome(&mut executor, &mut program);
        assert_eq!(result, Err("Division by zero at line 120".to_string()));
        let typed = type_line(&mut executor, &program, "PRINT X");
        assert_eq!(typed, Ok(RunOutcome::Ended));
        executor.get_output().to_string()
    });

    assert_eq!(output, "1\n");
}

#[test]
fn test_typed_goto_reaches_stored_line_that_does_not_parse() {
    let (results, output) = typed(
        &["10 PRINT \"A\"", "20 PRINT )(", "30 PRINT \"C\""],
        &["GOTO 10", "GOTO 20", "GOTO 30"],
    );

    for result in &results[..2] {
        let message = result.clone().expect_err("line 20 gives its parse error");
        assert!(
            message.starts_with("Syntax error at line 20:"),
            "message was {message:?}"
        );
    }
    assert_eq!(results[2], Ok(RunOutcome::Ended));
    assert_eq!(output, "A\nC\n");
}

#[test]
fn test_typed_goto_reads_data_of_stored_program() {
    let (results, output) = typed(
        &["10 READ A", "20 PRINT A", "30 DATA 42, 43"],
        &["GOTO 10", "READ B : PRINT B"],
    );

    assert_eq!(results, vec![Ok(RunOutcome::Ended); 2]);
    assert_eq!(output, "42\n43\n");
}

#[test]
fn test_typed_read_continues_after_run() {
    let output = with_limit(|| {
        let mut executor = Executor::new();
        let mut program = ProgramStore::new();
        store(&mut program, &["10 READ A", "20 PRINT A", "30 DATA 42, 43"]);
        assert_eq!(
            run_program_with_outcome(&mut executor, &mut program),
            Ok(RunOutcome::Ended)
        );
        let typed = type_line(&mut executor, &program, "READ B : PRINT B");
        assert_eq!(typed, Ok(RunOutcome::Ended));
        executor.get_output().to_string()
    });

    assert_eq!(output, "42\n43\n");
}

#[test]
fn test_changed_data_starts_again() {
    let output = with_limit(|| {
        let mut executor = Executor::new();
        let mut program = ProgramStore::new();
        store(&mut program, &["30 DATA 42, 43"]);
        let first = type_line(&mut executor, &program, "READ A : PRINT A");
        assert_eq!(first, Ok(RunOutcome::Ended));
        store(&mut program, &["30 DATA 7, 8"]);
        let second = type_line(&mut executor, &program, "READ A : PRINT A");
        assert_eq!(second, Ok(RunOutcome::Ended));
        executor.get_output().to_string()
    });

    assert_eq!(output, "42\n7\n");
}

#[test]
fn test_typed_goto_reaches_line_65535() {
    let (results, output) = typed(&["65535 PRINT \"LAST\""], &["GOTO 65535"]);

    assert_eq!(results, vec![Ok(RunOutcome::Ended)]);
    assert_eq!(output, "LAST\n");
}

#[test]
fn test_run_error_on_line_65535_gives_line_number() {
    let result = with_limit(|| {
        let mut executor = Executor::new();
        let mut program = ProgramStore::new();
        store(&mut program, &["65535 PRINT 1/0"]);
        run_program_with_outcome(&mut executor, &mut program)
    });

    assert_eq!(result, Err("Division by zero at line 65535".to_string()));
}

#[test]
fn test_typed_def_proc_gives_error() {
    let (results, output) = typed(&[], &["DEF PROCq:PRINT \"Q\":ENDPROC"]);

    assert_eq!(
        results,
        vec![Err("DEF PROC needs a line number".to_string())]
    );
    assert_eq!(output, "");
}

#[test]
fn test_typed_multi_line_def_fn_gives_error() {
    let (results, output) = typed(&[], &["PRINT \"A\" : DEF FNm"]);

    assert_eq!(results, vec![Err("DEF FN needs a line number".to_string())]);
    assert_eq!(output, "");
}

const PROGRAM_WITH_HANDLER: [&str; 4] = [
    "10 ON ERROR GOTO 100",
    "20 PRINT 1/0",
    "30 END",
    "100 PRINT \"HANDLED\"",
];

#[test]
fn test_error_in_stored_line_after_typed_goto_uses_handler() {
    let (results, output) = typed(&PROGRAM_WITH_HANDLER, &["GOTO 10"]);

    assert_eq!(results, vec![Ok(RunOutcome::Ended)]);
    assert_eq!(output, "HANDLED\n");
}

#[test]
fn test_error_in_typed_line_does_not_use_handler() {
    let (results, output) = typed(&PROGRAM_WITH_HANDLER, &["GOTO 10", "PRINT 1/0"]);

    assert_eq!(results[1], Err("Division by zero".to_string()));
    assert_eq!(output, "HANDLED\n");
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
fn test_binary_typed_for_loop_prints_each_value() {
    let (status, output) = run_binary(b"FOR I%=1 TO 3 : PRINT I% : NEXT\nEXIT\n");

    assert!(status.success(), "exit status was {status}");
    assert!(output.contains("> 1\n2\n3\n"), "output was {output:?}");
    assert!(!output.contains("Error"), "output was {output:?}");
}

#[test]
fn test_binary_typed_gosub_runs_stored_subroutine() {
    let (status, output) =
        run_binary(b"10 PRINT \"SUB\"\n20 RETURN\nGOSUB 10 : PRINT \"B\"\nEXIT\n");

    assert!(status.success(), "exit status was {status}");
    assert!(output.contains("SUB\nB\n"), "output was {output:?}");
    assert!(!output.contains("Error"), "output was {output:?}");
}

#[test]
fn test_binary_typed_nested_if_prints_z() {
    let input = "A%=1 : B%=0\nIF A% THEN IF B% THEN PRINT \"y\" ELSE PRINT \"z\"\n\
                 A%=0\nIF A% THEN IF B% THEN PRINT \"y\" ELSE PRINT \"z\"\nEXIT\n";
    let (status, output) = run_binary(input.as_bytes());

    assert!(status.success(), "exit status was {status}");
    assert_eq!(output.matches("z\n").count(), 2, "output was {output:?}");
    assert!(!output.contains("Error"), "output was {output:?}");
}

#[test]
fn test_binary_typed_quit_leaves_interpreter() {
    let (status, output) = run_binary(b"PRINT \"A\" : QUIT\nPRINT \"AFTER\"\nEXIT\n");

    assert!(status.success(), "exit status was {status}");
    assert!(output.contains("A\n"), "output was {output:?}");
    assert!(!output.contains("AFTER"), "output was {output:?}");
    assert!(output.contains("Goodbye!"), "output was {output:?}");
}
