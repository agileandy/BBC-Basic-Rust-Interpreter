//! Issue 17: IF blocks that cover more than one line (BBC BASIC V).
//!
//! A line `IF cond THEN` with nothing after THEN starts a block. A line that
//! starts with ELSE starts the alternative block. A line that starts with
//! ENDIF ends the block.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::{run_program, run_typed_line};
use bbc_basic_interpreter::tokenizer::tokenize;

/// Store the source lines, run the program and return the result and the output.
///
/// The program runs in a thread with a time limit, so a jump defect that
/// loops without end fails the test and does not hang the test run.
fn run(source: &[&str]) -> (Result<(), String>, String) {
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
        .expect("program stops in 5 seconds")
}

#[test]
fn test_block_with_no_else_runs_only_when_true() {
    for (value, expected) in [(1, "a\nb\nend\n"), (0, "end\n")] {
        let line_10 = format!("10 A%={}", value);
        let (result, output) = run(&[
            &line_10,
            "20 IF A% THEN",
            "30 PRINT \"a\"",
            "40 PRINT \"b\"",
            "50 ENDIF",
            "60 PRINT \"end\"",
        ]);

        assert_eq!(result, Ok(()), "A%={}", value);
        assert_eq!(output, expected, "A%={}", value);
    }
}

#[test]
fn test_block_with_else_runs_one_block() {
    for (value, expected) in [(1, "a\nb\nend\n"), (0, "c\nd\nend\n")] {
        let line_10 = format!("10 A%={}", value);
        let (result, output) = run(&[
            &line_10,
            "20 IF A% THEN",
            "30 PRINT \"a\"",
            "40 PRINT \"b\"",
            "50 ELSE",
            "60 PRINT \"c\"",
            "70 PRINT \"d\"",
            "80 ENDIF",
            "90 PRINT \"end\"",
        ]);

        assert_eq!(result, Ok(()), "A%={}", value);
        assert_eq!(output, expected, "A%={}", value);
    }
}

#[test]
fn test_nested_blocks_for_each_value_pair() {
    let expected = [
        ((0, 0), "y0\nend\n"),
        ((0, 1), "y1\nend\n"),
        ((1, 0), "x0\nend\n"),
        ((1, 1), "x1\nend\n"),
    ];
    for ((a, b), output_expected) in expected {
        let line_10 = format!("10 A%={} : B%={}", a, b);
        let (result, output) = run(&[
            &line_10,
            "20 IF A% THEN",
            "30 IF B% THEN",
            "40 PRINT \"x1\"",
            "50 ELSE",
            "60 PRINT \"x0\"",
            "70 ENDIF",
            "80 ELSE",
            "90 IF B% THEN",
            "100 PRINT \"y1\"",
            "110 ELSE",
            "120 PRINT \"y0\"",
            "130 ENDIF",
            "140 ENDIF",
            "150 PRINT \"end\"",
        ]);

        assert_eq!(result, Ok(()), "A%={} B%={}", a, b);
        assert_eq!(output, output_expected, "A%={} B%={}", a, b);
    }
}

#[test]
fn test_single_line_if_inside_block_is_not_a_block() {
    for (value, expected) in [(1, "x\ny\nend\n"), (0, "end\n")] {
        let line_10 = format!("10 A%={}", value);
        let (result, output) = run(&[
            &line_10,
            "20 IF A% THEN",
            "30 IF A% THEN PRINT \"x\" ELSE PRINT \"no\"",
            "40 IF 0 THEN PRINT \"no\"",
            "50 PRINT \"y\"",
            "60 ENDIF",
            "70 PRINT \"end\"",
        ]);

        assert_eq!(result, Ok(()), "A%={}", value);
        assert_eq!(output, expected, "A%={}", value);
    }
}

#[test]
fn test_block_inside_for_loop() {
    let (result, output) = run(&[
        "10 FOR I%=1 TO 4",
        "20 IF I% MOD 2 THEN",
        "30 PRINT \"odd \";I%",
        "40 ELSE",
        "50 PRINT \"even \";I%",
        "60 ENDIF",
        "70 NEXT",
        "80 PRINT \"end\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "odd 1\neven 2\nodd 3\neven 4\nend\n");
}

#[test]
fn test_goto_out_of_block_in_loop_three_times() {
    let (result, output) = run(&[
        "10 N%=0",
        "20 N%=N%+1",
        "30 IF N%<=3 THEN",
        "40 PRINT \"in \";N%",
        "50 GOTO 20",
        "60 ELSE",
        "70 PRINT \"else\"",
        "80 ENDIF",
        "90 PRINT \"end\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "in 1\nin 2\nin 3\nelse\nend\n");
}

#[test]
fn test_block_with_no_endif_gives_error_with_if_line() {
    for value in [0, 1] {
        let line_10 = format!("10 A%={}", value);
        for last_line in ["40 ELSE", "40 PRINT \"b\""] {
            let (result, _) = run(&[&line_10, "20 IF A% THEN", "30 PRINT \"a\"", last_line]);

            let error = result.expect_err("a block with no ENDIF gives an error");
            assert_eq!(
                error, "IF without matching ENDIF at line 20",
                "A%={} {}",
                value, last_line
            );
        }
    }

    // The ENDIF of the inner block does not close the outer block
    let (result, _) = run(&[
        "10 IF 1 THEN",
        "20 IF 0 THEN",
        "30 PRINT \"a\"",
        "40 ENDIF",
        "50 PRINT \"b\"",
    ]);
    assert_eq!(
        result,
        Err("IF without matching ENDIF at line 10".to_string())
    );
}

#[test]
fn test_block_in_procedure_and_multi_line_function() {
    let (result, output) = run(&[
        "10 PROCsign(-2) : PROCsign(3)",
        "20 PRINT FNbig(5);\" \";FNbig(50)",
        "30 END",
        "40 DEF PROCsign(X%)",
        "50 IF X%<0 THEN",
        "60 PRINT \"minus\"",
        "70 ELSE",
        "80 PRINT \"plus\"",
        "90 ENDIF",
        "100 ENDPROC",
        "110 DEF FNbig(X%)",
        "120 IF X%>10 THEN",
        "130 R%=1",
        "140 ELSE",
        "150 R%=0",
        "160 ENDIF",
        "170 =R%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "minus\nplus\n0 1\n");
}

#[test]
fn test_block_if_on_typed_line_gives_clear_error() {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut executor = Executor::new();
        let mut program = ProgramStore::new();
        program.store_line(tokenize("10 PRINT \"a\"").expect("line tokenizes"));
        program.store_line(tokenize("20 ENDIF").expect("line tokenizes"));
        let typed = tokenize("A%=1 : IF A% THEN").expect("typed line tokenizes");
        let result = run_typed_line(&mut executor, &program, &typed);
        let _ = sender.send((result, executor.get_output().to_string()));
    });
    let (result, output) = receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("typed line stops in 5 seconds");

    assert_eq!(result, Err("Block IF needs a line number".to_string()));
    assert_eq!(output, "");
}

#[test]
fn test_statements_after_else_and_endif_on_their_line() {
    for (value, expected) in [(1, "a\nz\nend\n"), (0, "c\nd\nz\nend\n")] {
        let line_10 = format!("10 A%={}", value);
        let (result, output) = run(&[
            &line_10,
            "20 IF A% THEN",
            "30 PRINT \"a\"",
            "40 ELSE PRINT \"c\" : PRINT \"d\"",
            "50 ENDIF : PRINT \"z\"",
            "60 PRINT \"end\"",
        ]);

        assert_eq!(result, Ok(()), "A%={}", value);
        assert_eq!(output, expected, "A%={}", value);
    }
}
