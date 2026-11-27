//! Issue 107: select one of many cases with CASE ... OF ... WHEN ... ENDCASE
//! (BBC BASIC V).
//!
//! A line that ends with `CASE expr OF` starts the structure. A line that
//! starts with `WHEN v1, v2 :` starts a case. A line that starts with
//! `OTHERWISE` starts the default case. A line that starts with `ENDCASE`
//! ends the structure. The first WHEN with a value equal to the CASE value
//! runs, then the run continues after ENDCASE.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::common::run_binary_read_first as run_binary;
use crate::common::run_timed_stops as run;
use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::{run_typed_line, RunOutcome};
use bbc_basic_interpreter::tokenizer::tokenize;

#[test]
fn test_number_case_runs_only_the_matching_when() {
    let expected = [
        (1, "one\nend\n"),
        (2, "two\nstill two\nend\n"),
        (3, "three\nend\n"),
        (4, "end\n"),
    ];
    for (value, output_expected) in expected {
        let line_10 = format!("10 X = {}", value);
        let (result, output) = run(&[
            &line_10,
            "20 CASE X OF",
            "30 WHEN 1 : PRINT \"one\"",
            "40 WHEN 2 : PRINT \"two\"",
            "50 PRINT \"still two\"",
            "60 WHEN 3",
            "70 PRINT \"three\"",
            "80 ENDCASE",
            "90 PRINT \"end\"",
        ]);

        assert_eq!(result, Ok(()), "X = {}", value);
        assert_eq!(output, output_expected, "X = {}", value);
    }
}

#[test]
fn test_string_case_runs_the_matching_when() {
    for (value, expected) in [("cat", "meow\n"), ("dog", "woof\n"), ("cow", "")] {
        let line_10 = format!("10 A$ = \"{}\"", value);
        let (result, output) = run(&[
            &line_10,
            "20 CASE A$ OF",
            "30 WHEN \"cat\" : PRINT \"meow\"",
            "40 WHEN \"d\" + \"og\" : PRINT \"woof\"",
            "50 ENDCASE",
        ]);

        assert_eq!(result, Ok(()), "A$ = {}", value);
        assert_eq!(output, expected, "A$ = {}", value);
    }
}

#[test]
fn test_numbers_compare_as_numbers() {
    let (result, output) = run(&[
        "10 CASE 2 OF",
        "20 WHEN 2.0 : PRINT \"two\"",
        "30 ENDCASE",
        "40 CASE 1.5 OF",
        "50 WHEN 1 : PRINT \"one\"",
        "60 WHEN 3 / 2 : PRINT \"one and a half\"",
        "70 ENDCASE",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "two\none and a half\n");
}

#[test]
fn test_when_with_three_values_matches_each_value() {
    for value in [1, 2, 3] {
        let line_10 = format!("10 X% = {}", value);
        let (result, output) = run(&[
            &line_10,
            "20 CASE X% OF",
            "30 WHEN 0 : PRINT \"zero\"",
            "40 WHEN 1, 2, 3 : PRINT \"small\"",
            "50 WHEN 2 : PRINT \"two\"",
            "60 ENDCASE",
        ]);

        assert_eq!(result, Ok(()), "X% = {}", value);
        assert_eq!(output, "small\n", "X% = {}", value);
    }
}

#[test]
fn test_when_value_can_be_a_function_with_two_arguments() {
    let (result, output) = run(&[
        "10 CASE 3 OF",
        "20 WHEN FNadd(1, 2), 7 : PRINT \"three\"",
        "30 ENDCASE",
        "40 END",
        "50 DEF FNadd(A, B) = A + B",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "three\n");
}

#[test]
fn test_otherwise_runs_when_no_value_matches() {
    for (value, expected) in [(1, "one\nend\n"), (5, "other\nmore\nend\n")] {
        let line_10 = format!("10 X = {}", value);
        let (result, output) = run(&[
            &line_10,
            "20 CASE X OF",
            "30 WHEN 1 : PRINT \"one\"",
            "40 OTHERWISE PRINT \"other\"",
            "50 PRINT \"more\"",
            "60 ENDCASE : PRINT \"end\"",
        ]);

        assert_eq!(result, Ok(()), "X = {}", value);
        assert_eq!(output, expected, "X = {}", value);
    }
}

#[test]
fn test_no_match_and_no_otherwise_runs_nothing() {
    let (result, output) = run(&[
        "10 CASE 9 OF",
        "20 WHEN 1 : PRINT \"one\"",
        "30 WHEN 2 : PRINT \"two\"",
        "40 ENDCASE",
        "50 PRINT \"end\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "end\n");
}

#[test]
fn test_case_value_is_evaluated_one_time() {
    let (result, output) = run(&[
        "10 C% = 0",
        "20 CASE FNnext OF",
        "30 WHEN 5 : PRINT \"five\"",
        "40 WHEN 6 : PRINT \"six\"",
        "50 WHEN 1 : PRINT \"one\"",
        "60 ENDCASE",
        "70 PRINT C%",
        "80 END",
        "90 DEF FNnext",
        "100 C% = C% + 1",
        "110 = C%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "one\n1\n");
}

#[test]
fn test_string_and_number_pair_is_type_mismatch() {
    let (result, output) = run(&[
        "10 CASE 1 OF",
        "20 WHEN \"one\" : PRINT \"one\"",
        "30 ENDCASE",
    ]);

    assert_eq!(result, Err("Type mismatch at line 20".to_string()));
    assert_eq!(output, "");
}

#[test]
fn test_type_mismatch_goes_to_the_error_handler() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 50",
        "20 CASE \"a\" OF",
        "30 WHEN 1 : PRINT \"one\"",
        "40 ENDCASE",
        "50 PRINT \"error \"; ERR; \" at \"; ERL",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "error 6 at 30\n");
}

#[test]
fn test_nested_case_for_each_value_pair() {
    let expected = [
        ((1, 1), "a1\nend\n"),
        ((1, 2), "a2\nend\n"),
        ((1, 3), "a other\nend\n"),
        ((2, 1), "b\nend\n"),
        ((3, 1), "other\nend\n"),
    ];
    for ((a, b), output_expected) in expected {
        let line_10 = format!("10 A% = {} : B% = {}", a, b);
        let (result, output) = run(&[
            &line_10,
            "20 CASE A% OF",
            "30 WHEN 1 : CASE B% OF",
            "40 WHEN 1 : PRINT \"a1\"",
            "50 WHEN 2 : PRINT \"a2\"",
            "60 OTHERWISE PRINT \"a other\"",
            "70 ENDCASE",
            "80 WHEN 2 : PRINT \"b\"",
            "90 OTHERWISE",
            "100 CASE B% OF",
            "110 WHEN 9 : PRINT \"nine\"",
            "120 ENDCASE",
            "130 PRINT \"other\"",
            "140 ENDCASE",
            "150 PRINT \"end\"",
        ]);

        assert_eq!(result, Ok(()), "A% = {}, B% = {}", a, b);
        assert_eq!(output, output_expected, "A% = {}, B% = {}", a, b);
    }
}

#[test]
fn test_block_if_in_a_case() {
    let expected = [
        ((1, 1), "yes\nend\n"),
        ((1, 0), "no\nend\n"),
        ((2, 1), "two\nend\n"),
    ];
    for ((a, b), output_expected) in expected {
        let line_10 = format!("10 A% = {} : B% = {}", a, b);
        let (result, output) = run(&[
            &line_10,
            "20 CASE A% OF",
            "30 WHEN 1",
            "40 IF B% THEN",
            "50 PRINT \"yes\"",
            "60 ELSE",
            "70 PRINT \"no\"",
            "80 ENDIF",
            "90 WHEN 2 : IF B% THEN",
            "100 PRINT \"two\"",
            "110 ENDIF",
            "120 ENDCASE",
            "130 PRINT \"end\"",
        ]);

        assert_eq!(result, Ok(()), "A% = {}, B% = {}", a, b);
        assert_eq!(output, output_expected, "A% = {}, B% = {}", a, b);
    }
}

#[test]
fn test_case_in_a_block_if() {
    let expected = [
        ((1, 1), "one\nend\n"),
        ((1, 2), "two\nend\n"),
        ((0, 1), "else one\nend\n"),
        ((0, 2), "else\nend\n"),
    ];
    for ((a, b), output_expected) in expected {
        let line_10 = format!("10 A% = {} : B% = {}", a, b);
        let (result, output) = run(&[
            &line_10,
            "20 IF A% THEN",
            "30 CASE B% OF",
            "40 WHEN 1 : PRINT \"one\"",
            "50 OTHERWISE PRINT \"two\"",
            "60 ENDCASE",
            "70 ELSE CASE B% OF",
            "80 WHEN 1 : PRINT \"else one\"",
            "90 OTHERWISE PRINT \"else\"",
            "100 ENDCASE",
            "110 ENDIF",
            "120 PRINT \"end\"",
        ]);

        assert_eq!(result, Ok(()), "A% = {}, B% = {}", a, b);
        assert_eq!(output, output_expected, "A% = {}, B% = {}", a, b);
    }
}

#[test]
fn test_case_in_a_for_loop() {
    let (result, output) = run(&[
        "10 FOR I% = 1 TO 4",
        "20 CASE I% OF",
        "30 WHEN 1, 3 : PRINT \"odd\"",
        "40 WHEN 2 : PRINT \"two\"",
        "50 OTHERWISE PRINT \"four\"",
        "60 ENDCASE",
        "70 NEXT I%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "odd\ntwo\nodd\nfour\n");
}

#[test]
fn test_case_in_a_procedure() {
    let (result, output) = run(&[
        "10 PROCname(1) : PROCname(2) : PROCname(3)",
        "20 END",
        "30 DEF PROCname(N%)",
        "40 CASE N% OF",
        "50 WHEN 1 : PRINT \"one\"",
        "60 WHEN 2 : PRINT \"two\" : ENDPROC",
        "70 OTHERWISE PRINT \"many\"",
        "80 ENDCASE",
        "90 PRINT \"after\"",
        "100 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "one\nafter\ntwo\nmany\nafter\n");
}

#[test]
fn test_case_in_a_multi_line_function() {
    let (result, output) = run(&[
        "10 PRINT FNname$(1); \" \"; FNname$(2); \" \"; FNname$(7)",
        "20 END",
        "30 DEF FNname$(N%)",
        "40 LOCAL R$",
        "50 CASE N% OF",
        "60 WHEN 1 : R$ = \"one\"",
        "70 WHEN 2 : R$ = \"two\"",
        "80 OTHERWISE R$ = \"many\"",
        "90 ENDCASE",
        "100 = R$",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "one two many\n");
}

#[test]
fn test_goto_out_of_a_case_needs_no_cleanup() {
    let (result, output) = run(&[
        "10 C% = 0",
        "20 C% = C% + 1",
        "30 CASE C% OF",
        "40 WHEN 5000 : GOTO 80",
        "50 OTHERWISE GOTO 20",
        "60 ENDCASE",
        "70 PRINT \"not here\"",
        "80 PRINT C%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "5000\n");
}

#[test]
fn test_missing_endcase_is_an_error_before_the_run() {
    let (result, output) = run(&[
        "10 PRINT \"start\"",
        "20 CASE 1 OF",
        "30 WHEN 1 : PRINT \"one\"",
    ]);

    assert_eq!(
        result,
        Err("CASE without matching ENDCASE at line 20".to_string())
    );
    assert_eq!(output, "");
}

#[test]
fn test_missing_inner_endcase_is_an_error_before_the_run() {
    let (result, output) = run(&[
        "10 CASE 1 OF",
        "20 WHEN 1 : CASE 2 OF",
        "30 WHEN 2 : PRINT \"two\"",
        "40 ENDCASE",
    ]);

    assert_eq!(
        result,
        Err("CASE without matching ENDCASE at line 10".to_string())
    );
    assert_eq!(output, "");
}

#[test]
fn test_when_otherwise_and_endcase_with_no_case_are_errors_before_the_run() {
    for (line, word) in [
        ("20 WHEN 1 : PRINT \"one\"", "WHEN"),
        ("20 OTHERWISE PRINT \"one\"", "OTHERWISE"),
        ("20 ENDCASE", "ENDCASE"),
    ] {
        let (result, output) = run(&["10 PRINT \"start\"", line, "30 PRINT \"end\""]);

        assert_eq!(
            result,
            Err(format!("{} without matching CASE at line 20", word)),
            "{}",
            word
        );
        assert_eq!(output, "", "{}", word);
    }
}

#[test]
fn test_text_after_of_is_a_syntax_error() {
    let (result, output) = run(&[
        "10 CASE 1 OF : PRINT \"one\"",
        "20 WHEN 1 : PRINT \"one\"",
        "30 ENDCASE",
    ]);

    assert_eq!(
        result,
        Err("Syntax error at line 10: CASE must be the last statement of a line".to_string())
    );
    assert_eq!(output, "");
}

#[test]
fn test_when_not_at_the_start_of_a_line_is_a_syntax_error() {
    let (result, _) = run(&[
        "10 CASE 1 OF",
        "20 PRINT 1 : WHEN 1 : PRINT \"one\"",
        "30 ENDCASE",
    ]);

    assert_eq!(
        result,
        Err("Syntax error at line 20: WHEN must be the first statement of a line".to_string())
    );
}

#[test]
fn test_lower_case_names_are_variables_and_arrays() {
    let (result, output) = run(&[
        "10 when = 1 : case = 2 : of = 3 : otherwise = 4 : endcase = 5",
        "20 PRINT when + case + of + otherwise + endcase",
        "30 DIM when(2), of(2), endcase(2)",
        "40 when(1) = 6 : of(2) = 7 : endcase(0) = 8",
        "50 PRINT when(1) + of(2) + endcase(0)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "15\n21\n");
}

/// Store the program lines, then run one typed line, with a time limit.
fn typed(program_lines: &[&str], typed_line: &str) -> (Result<RunOutcome, String>, String) {
    let program_lines: Vec<String> = program_lines.iter().map(|l| l.to_string()).collect();
    let typed_line = typed_line.to_string();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut executor = Executor::new();
        let mut program = ProgramStore::new();
        for line in &program_lines {
            program.store_line(tokenize(line).expect("line tokenizes"));
        }
        let typed = tokenize(&typed_line).expect("typed line tokenizes");
        let result = run_typed_line(&mut executor, &program, &typed);
        let _ = sender.send((result, executor.get_output().to_string()));
    });
    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("typed line stops in 5 seconds")
}

#[test]
fn test_typed_case_needs_a_line_number() {
    let (result, output) = typed(&[], "CASE 1 OF");

    assert_eq!(result, Err("CASE needs a line number".to_string()));
    assert_eq!(output, "");
}

#[test]
fn test_typed_when_and_otherwise_are_errors() {
    for (line, word) in [
        ("WHEN 1 : PRINT 1", "WHEN"),
        ("OTHERWISE PRINT 1", "OTHERWISE"),
    ] {
        let (result, output) = typed(&[], line);

        assert_eq!(
            result,
            Err(format!("{} without matching CASE", word)),
            "{}",
            word
        );
        assert_eq!(output, "", "{}", word);
    }
}

#[test]
fn test_binary_lists_and_runs_a_case() {
    let output = run_binary(concat!(
        "10 FOR I% = 1 TO 3\n",
        "20 CASE I% OF\n",
        "30 WHEN 1\n",
        "35 PRINT \"low\"\n",
        "40 OTHERWISE PRINT \"high\"\n",
        "50 ENDCASE\n",
        "60 NEXT I%\n",
        "LIST\n",
        "RUN\n",
        "EXIT\n",
    ));

    assert!(
        output.contains(concat!(
            "10 FOR I% = 1 TO 3\n",
            "20 CASE I% OF\n",
            "30 WHEN 1\n",
            "35 PRINT \"low\"\n",
            "40 OTHERWISE PRINT \"high\"\n",
            "50 ENDCASE\n",
            "60 NEXT I%\n",
        )),
        "LIST output: {}",
        output
    );
    assert!(
        output.contains("low\nhigh\nhigh\n"),
        "RUN output: {}",
        output
    );
}

// Review finding K1: an error in a WHEN value has the line of the WHEN.

#[test]
fn test_error_in_a_when_value_has_the_line_of_the_when() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 X = 1",
        "30 CASE X OF",
        "40 WHEN 2 : PRINT \"two\"",
        "50 WHEN 1 / 0 : PRINT \"bad\"",
        "60 ENDCASE",
        "70 END",
        "100 PRINT \"err at \"; ERL",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "err at 50\n");
}

#[test]
fn test_error_in_a_when_value_with_no_handler_has_the_line_of_the_when() {
    let (result, output) = run(&[
        "30 CASE 1 OF",
        "40 WHEN 2 : PRINT \"two\"",
        "50 WHEN 1 / 0 : PRINT \"bad\"",
        "60 ENDCASE",
    ]);

    assert_eq!(result, Err("Division by zero at line 50".to_string()));
    assert_eq!(output, "");
}

#[test]
fn test_error_in_a_function_body_of_a_when_value_has_the_line_of_the_body() {
    let (result, _) = run(&[
        "30 CASE 1 OF",
        "40 WHEN FNbad : PRINT \"bad\"",
        "50 ENDCASE",
        "60 END",
        "70 DEF FNbad",
        "80 = 1 / 0",
    ]);

    assert_eq!(result, Err("Division by zero at line 80".to_string()));
}

// Review finding K2: a stored CASE line with a syntax error must not block a
// typed command.

#[test]
fn test_typed_command_runs_when_a_stored_case_line_is_invalid() {
    let (result, output) = typed(
        &[
            "10 X = 1",
            "20 CASE X+ OF",
            "30 WHEN 1 : PRINT \"one\"",
            "50 ENDCASE",
        ],
        "PRINT 1",
    );

    assert_eq!(result, Ok(RunOutcome::Ended));
    assert_eq!(output, "1\n");
}

#[test]
fn test_typed_command_runs_when_a_stored_endcase_is_missing() {
    let (result, output) = typed(&["10 CASE 1 OF", "20 WHEN 1 : PRINT \"one\""], "PRINT 2");

    assert_eq!(result, Ok(RunOutcome::Ended));
    assert_eq!(output, "2\n");
}

#[test]
fn test_typed_goto_to_a_case_with_no_endcase_is_an_error() {
    let (result, output) = typed(&["10 CASE 1 OF", "20 WHEN 1 : PRINT \"one\""], "GOTO 10");

    assert_eq!(
        result,
        Err("CASE without matching ENDCASE at line 10".to_string())
    );
    assert_eq!(output, "");
}

#[test]
fn test_run_shows_the_syntax_error_of_an_invalid_case_line() {
    let (result, output) = run(&[
        "10 X = 1",
        "20 CASE X+ OF",
        "30 WHEN 1 : PRINT \"one\"",
        "50 ENDCASE",
    ]);

    assert_eq!(
        result,
        Err("Syntax error at line 20: Unexpected end of expression".to_string())
    );
    assert_eq!(output, "");
}

// Review finding K3: upper-case names OF, WHEN, OTHERWISE and ENDCASE are
// still variables and arrays before `=` (rule 15).

#[test]
fn test_upper_case_names_are_variables_and_arrays() {
    let (result, output) = run(&[
        "10 WHEN=5",
        "20 OF=2",
        "30 OTHERWISE=1 : ENDCASE=3",
        "40 PRINT WHEN+OF+OTHERWISE+ENDCASE",
        "50 DIM WHEN(3), OTHERWISE(2), ENDCASE(2), OF(2)",
        "60 WHEN(3)=7 : OTHERWISE(1)=4",
        "70 ENDCASE(2)=1 : OF(1)=2",
        "80 PRINT WHEN(3)+OTHERWISE(1)+ENDCASE(2)+OF(1)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "11\n14\n");
}

#[test]
fn test_upper_case_variables_in_a_case_structure() {
    let (result, output) = run(&[
        "10 WHEN=2 : OF=2",
        "20 CASE OF OF",
        "30 WHEN WHEN : PRINT \"match\"",
        "40 OTHERWISE PRINT \"no match\"",
        "50 ENDCASE",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "match\n");
}

#[test]
fn test_when_value_can_be_a_comparison() {
    let (result, output) = run(&[
        "10 A = 3",
        "20 CASE TRUE OF",
        "30 WHEN A = 1 : PRINT \"one\"",
        "40 WHEN A > 2 : PRINT \"big\"",
        "50 ENDCASE",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "big\n");
}

// Review finding K5: an empty statement or a REM can follow OF.

#[test]
fn test_empty_statement_and_rem_after_of() {
    for line_20 in [
        "20 CASE X OF :",
        "20 CASE X OF REM pick",
        "20 CASE X OF : REM pick",
    ] {
        let (result, output) = run(&[
            "10 X = 2",
            line_20,
            "30 WHEN 1 : PRINT \"one\"",
            "40 WHEN 2 : PRINT \"two\"",
            "50 ENDCASE",
        ]);

        assert_eq!(result, Ok(()), "{}", line_20);
        assert_eq!(output, "two\n", "{}", line_20);
    }
}

#[test]
fn test_nested_case_line_with_rem_after_of() {
    let (result, output) = run(&[
        "10 CASE 1 OF",
        "20 WHEN 1 : CASE 2 OF REM inner",
        "30 WHEN 2 : PRINT \"two\"",
        "40 ENDCASE",
        "50 WHEN 3 : PRINT \"three\"",
        "60 ENDCASE",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "two\n");
}

// Review finding K4: LIST shows a space before OF after a bracket.

#[test]
fn test_binary_lists_a_space_before_of_after_a_bracket() {
    let output = run_binary(concat!(
        "10 CASE LEN(\"ab\") OF\n",
        "20 WHEN 2\n",
        "30 PRINT \"two\"\n",
        "40 ENDCASE\n",
        "LIST\n",
        "RUN\n",
        "EXIT\n",
    ));

    assert!(
        output.contains("10 CASE LEN(\"ab\") OF\n"),
        "LIST output: {}",
        output
    );
    assert!(output.contains("two\n"), "RUN output: {}", output);
}

#[test]
fn test_case_on_the_endcase_line_of_the_case_before() {
    let (result, output) = run(&[
        "10 CASE 1 OF",
        "20 WHEN 1 : PRINT \"first\"",
        "30 ENDCASE : CASE 2 OF",
        "40 WHEN 2 : PRINT \"second\"",
        "50 ENDCASE",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "first\nsecond\n");
}
