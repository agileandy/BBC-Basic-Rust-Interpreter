//! Tests that run complete BASIC programs through the library run loop.

use crate::common::run_lines as run;

#[test]
fn test_for_loop_runs_body_for_each_value() {
    let (result, output) = run(&[
        "10 FOR I% = 1 TO 3",
        "20 PRINT \"X\"",
        "30 NEXT I%",
        "40 PRINT \"DONE\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "X\nX\nX\nDONE\n");
}

#[test]
fn test_empty_program_gives_error() {
    let (result, output) = run(&[]);

    assert_eq!(result, Err("No program to run".to_string()));
    assert_eq!(output, "");
}

#[test]
fn test_goto_skips_lines() {
    let (result, output) = run(&["10 GOTO 30", "20 PRINT \"SKIPPED\"", "30 PRINT \"TARGET\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "TARGET\n");
}

#[test]
fn test_goto_missing_line_gives_error() {
    let (result, _) = run(&["10 GOTO 99"]);

    assert_eq!(result, Err("No such line: 99 at line 10".to_string()));
}

#[test]
fn test_gosub_returns_to_line_after_call() {
    let (result, output) = run(&[
        "10 GOSUB 100",
        "20 PRINT \"BACK\"",
        "30 END",
        "100 PRINT \"SUB\"",
        "110 RETURN",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "SUB\nBACK\n");
}

#[test]
fn test_repeat_until_runs_until_condition_is_true() {
    let (result, output) = run(&[
        "10 N% = 0",
        "20 REPEAT",
        "30 PRINT \"R\"",
        "40 N% = N% + 1",
        "50 UNTIL N% = 2",
        "60 PRINT \"DONE\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "R\nR\nDONE\n");
}

#[test]
fn test_while_loops_while_condition_is_true() {
    let (result, output) = run(&[
        "10 N% = 0",
        "20 WHILE N% < 2",
        "30 PRINT \"W\"",
        "40 N% = N% + 1",
        "50 ENDWHILE",
        "60 PRINT \"DONE\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "W\nW\nDONE\n");
}

#[test]
fn test_false_while_skips_nested_loop_body() {
    let (result, output) = run(&[
        "10 N% = 9",
        "20 WHILE N% < 2",
        "30 WHILE N% < 1",
        "40 PRINT \"INNER\"",
        "50 ENDWHILE",
        "60 PRINT \"OUTER\"",
        "70 ENDWHILE",
        "80 PRINT \"DONE\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "DONE\n");
}

#[test]
fn test_procedure_receives_arguments() {
    let (result, output) = run(&[
        "10 PROC show(\"A\", \"B\")",
        "20 PRINT \"BACK\"",
        "30 END",
        "100 DEF PROC show(P$, Q$)",
        "110 PRINT Q$; P$",
        "120 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "BA\nBACK\n");
}

#[test]
fn test_on_error_jumps_to_handler() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 X = 1 / 0",
        "30 PRINT \"NOT REACHED\"",
        "40 END",
        "100 PRINT \"CAUGHT\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "CAUGHT\n");
}

#[test]
fn test_error_without_handler_stops_program() {
    let (result, output) = run(&["10 PRINT \"START\"", "20 X = 1 / 0", "30 PRINT \"END\""]);

    assert!(
        result.is_err(),
        "expected a runtime error, got {:?}",
        result
    );
    assert_eq!(output, "START\n");
}

#[test]
fn test_each_example_program_runs_without_error() {
    let examples = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut failures = Vec::new();
    let mut count = 0;

    for entry in std::fs::read_dir(&examples).expect("examples directory exists") {
        let path = entry.expect("directory entry is readable").path();
        if path.extension().and_then(|e| e.to_str()) != Some("bas") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        count += 1;
        let source = std::fs::read_to_string(&path).expect("example is readable");
        // Only numbered lines are program lines
        let lines: Vec<&str> = source
            .lines()
            .filter(|line| line.trim_start().starts_with(|c: char| c.is_ascii_digit()))
            .collect();
        match std::panic::catch_unwind(|| run(&lines).0) {
            Ok(Ok(())) => {}
            Ok(Err(e)) => failures.push(format!("{}: {}", name, e)),
            Err(_) => failures.push(format!("{}: panic", name)),
        }
    }

    assert!(count > 0, "no example programs found");
    assert!(
        failures.is_empty(),
        "examples failed:\n{}",
        failures.join("\n")
    );
}

#[test]
fn test_data_is_read_before_its_line_runs() {
    let (result, output) = run(&["10 READ A$", "20 PRINT A$", "30 DATA \"HELLO\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "HELLO\n");
}

#[test]
fn test_def_fn_with_parameters_returns_value() {
    let (result, output) = run(&[
        "10 DEF FN add(X, Y) = X + Y",
        "20 IF FN add(5, 3) = 8 THEN PRINT \"OK\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "OK\n");
}

#[test]
fn test_print_pi_gives_its_value() {
    let (result, output) = run(&["10 PRINT PI"]);

    assert_eq!(result, Ok(()));
    assert!(output.starts_with("3.14159"), "output was {:?}", output);
}

#[test]
fn test_error_handler_can_print_err_and_erl() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 X = 1 / 0",
        "30 END",
        "100 PRINT ERR",
        "110 PRINT ERL",
    ]);

    assert_eq!(result, Ok(()));
    // 18 is the BBC BASIC error number for "Division by zero"
    assert_eq!(output, "18\n20\n");
}

#[test]
fn test_string_function_result_can_be_joined() {
    let (result, output) = run(&[
        "10 N% = 7",
        "20 S$ = \"A\" + STR$(N%) + \"B\"",
        "30 PRINT S$",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "A7B\n");
}

#[test]
fn test_negative_tab_and_spc_print_no_spaces() {
    let (result, output) = run(&["10 PRINT TAB(-5); \"*\"", "20 PRINT SPC(-5); \"+\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "*\n+\n");
}

#[test]
fn test_goto_in_if_jumps_when_condition_is_true() {
    let (result, output) = run(&[
        "10 X% = 1",
        "20 IF X% = 1 THEN GOTO 40",
        "30 PRINT \"SKIPPED\"",
        "40 PRINT \"TARGET\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "TARGET\n");
}

#[test]
fn test_end_in_else_branch_stops_program() {
    let (result, output) = run(&[
        "10 X% = 1",
        "20 IF X% = 2 THEN PRINT \"THEN\" ELSE END",
        "30 PRINT \"NOT REACHED\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "");
}

#[test]
fn test_on_goto_index_out_of_range_gives_on_range_error() {
    // Issue 66: with no ELSE, BBC BASIC gives the error "ON range" (ERR 40)
    let (result, output) = run(&[
        "10 X% = 5",
        "20 ON X% GOTO 40, 50",
        "30 PRINT \"NEXT LINE\"",
        "40 END",
        "50 END",
    ]);

    assert!(result.is_err(), "result: {:?}", result);
    assert_eq!(output, "");
}

#[test]
fn test_doubled_quote_in_string_gives_one_quote() {
    let (result, output) = run(&["10 PRINT \"SAY \"\"HI\"\" NOW\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "SAY \"HI\" NOW\n");
}

#[test]
fn test_nested_for_loops_run_inner_loop_for_each_outer_value() {
    let (result, output) = run(&[
        "10 FOR I% = 1 TO 2",
        "20 FOR J% = 1 TO 2",
        "30 PRINT \"J\"",
        "40 NEXT J%",
        "50 PRINT \"I\"",
        "60 NEXT I%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "J\nJ\nI\nJ\nJ\nI\n");
}

#[test]
fn test_val_reads_a_real_number() {
    let (result, output) = run(&[
        "10 PRINT VAL(\"3.5\")",
        "20 X% = VAL(\"3.5\")",
        "30 IF X% = 3 THEN PRINT \"TRUNCATED\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "3.5\nTRUNCATED\n");
}

#[test]
fn test_proc_and_fn_names_can_join_the_keyword() {
    let (result, output) = run(&[
        "10 DEF FNdouble(N) = N * 2",
        "20 PROCgreet",
        "30 IF FNdouble(4) = 8 THEN PRINT \"FN OK\"",
        "40 END",
        "100 DEF PROCgreet",
        "110 PRINT \"HELLO\"",
        "120 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "HELLO\nFN OK\n");
}

#[test]
fn test_if_compares_fraction_parts_of_real_values() {
    let (result, output) = run(&[
        "10 X = 6.283",
        "20 IF X > 6.28 THEN PRINT \"GREATER\" ELSE PRINT \"NOT GREATER\"",
        "30 IF X = 6.5 THEN PRINT \"EQUAL\" ELSE PRINT \"NOT EQUAL\"",
        "40 IF 0.5 < 0.9 THEN PRINT \"LESS\" ELSE PRINT \"NOT LESS\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "GREATER\nNOT EQUAL\nLESS\n");
}

#[test]
fn test_until_compares_real_values() {
    let (result, output) = run(&[
        "10 X = 0",
        "20 REPEAT",
        "30 X = X + 0.25",
        "40 PRINT \"STEP\"",
        "50 UNTIL X >= 0.75",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "STEP\nSTEP\nSTEP\n");
}

#[test]
fn test_comparison_mixes_integer_and_real_expressions() {
    let (result, output) = run(&[
        "10 A% = 7",
        "20 IF A% / 2 > 3.2 THEN PRINT \"REAL DIVIDE\"",
        "30 IF A% MOD 4 + 0.5 > 3.2 THEN PRINT \"MOD\"",
        "40 IF INT(2.9) + 0.5 < 2.6 THEN PRINT \"INT\"",
        "50 IF SQR(2) > 1.4 AND SQR(2) < 1.5 THEN PRINT \"SQR\"",
        "60 IF A% = 7 THEN PRINT \"INTEGER\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "REAL DIVIDE\nMOD\nINT\nSQR\nINTEGER\n");
}
