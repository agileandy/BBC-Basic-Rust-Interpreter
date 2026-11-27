//! Tests for functions that have more than one line (issue 6).

use crate::common::run_lines as run;
use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::parser::parse_statement;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;

#[test]
fn test_function_with_local_gives_result_and_keeps_caller_variable() {
    let (result, output) = run(&[
        "10 A = 5",
        "20 PRINT FNsum(3)",
        "30 PRINT A",
        "40 END",
        "100 DEF FNsum(N)",
        "110 LOCAL A",
        "120 A = N * 2",
        "130 A = A + 1",
        "140 = A",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "7\n5\n");
}

#[test]
fn test_recursive_factorial_gives_120_for_5() {
    let (result, output) = run(&[
        "10 PRINT FNfact(5)",
        "20 END",
        "100 DEF FNfact(N)",
        "110 IF N <= 1 THEN =1",
        "120 = N * FNfact(N - 1)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "120\n");
}

#[test]
fn test_function_with_for_loop_in_body() {
    let (result, output) = run(&[
        "10 PRINT FNsum%(4)",
        "20 PRINT \"DONE\"",
        "30 END",
        "100 DEF FNsum%(N%)",
        "110 LOCAL I%, T%",
        "120 FOR I% = 1 TO N%",
        "130 T% = T% + I%",
        "140 NEXT I%",
        "150 = T%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "10\nDONE\n");
}

#[test]
fn test_function_calls_procedure_and_procedure_calls_function() {
    let (result, output) = run(&[
        "10 PRINT FNouter(2)",
        "20 PROCshow(3)",
        "30 END",
        "100 DEF FNouter(X)",
        "110 PROCnote",
        "120 = X * 10",
        "200 DEF PROCnote",
        "210 PRINT \"IN PROC\"",
        "220 ENDPROC",
        "300 DEF PROCshow(Y)",
        "310 PRINT FNtwice(Y)",
        "320 ENDPROC",
        "400 DEF FNtwice(Z)",
        "410 Z = Z * 2",
        "420 = Z",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "IN PROC\n20\n6\n");
}

#[test]
fn test_string_function_with_more_than_one_line() {
    let (result, output) = run(&[
        "10 PRINT FNgreet$(\"BOB\")",
        "20 END",
        "100 DEF FNgreet$(N$)",
        "110 LOCAL G$",
        "120 G$ = \"HELLO \"",
        "130 G$ = G$ + N$",
        "140 = G$",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "HELLO BOB\n");
}

#[test]
fn test_error_in_function_body_reaches_on_error_with_body_line() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 50",
        "20 PRINT FNbad(1)",
        "30 PRINT \"NOT HERE\"",
        "40 END",
        "50 PRINT \"ERL \"; ERL",
        "60 END",
        "100 DEF FNbad(X)",
        "110 X = X + 1",
        "120 X = X / 0",
        "130 = X",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "ERL 120\n");
}

#[test]
fn test_error_in_function_body_stops_program_with_body_line() {
    let (result, output) = run(&[
        "10 A = FNbad(1)",
        "20 PRINT \"NOT HERE\"",
        "30 END",
        "100 DEF FNbad(X)",
        "110 = X / 0",
    ]);

    assert_eq!(result, Err("Division by zero at line 110".to_string()));
    assert_eq!(output, "");
}

#[test]
fn test_function_call_in_if_condition_and_in_print() {
    let (result, output) = run(&[
        "10 IF FNbig(7) THEN PRINT \"BIG\" ELSE PRINT \"SMALL\"",
        "20 IF FNbig(2) THEN PRINT \"BIG\" ELSE PRINT \"SMALL\"",
        "30 PRINT \"VALUE \"; FNbig(9)",
        "40 END",
        "100 DEF FNbig(V)",
        "110 IF V > 5 THEN =-1",
        "120 = 0",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "BIG\nSMALL\nVALUE -1\n");
}

#[test]
fn test_string_function_in_print_runs_its_body_one_time() {
    let (result, output) = run(&[
        "10 PRINT FNname$",
        "20 END",
        "100 DEF FNname$",
        "110 PRINT \"BODY\"",
        "120 = \"ANN\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "BODY\nANN\n");
}

#[test]
fn test_end_in_function_body_stops_program() {
    let (result, output) = run(&[
        "10 A = FNquit",
        "20 PRINT \"NOT HERE\"",
        "30 END",
        "100 DEF FNquit",
        "110 PRINT \"BYE\"",
        "120 END",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "BYE\n");
}

#[test]
fn test_main_run_reaching_function_end_line_gives_error() {
    let (result, output) = run(&["10 DEF FNx", "20 PRINT \"BODY\"", "30 = 1"]);

    assert_eq!(result, Err("= without FN call at line 30".to_string()));
    assert_eq!(output, "BODY\n");
}

#[test]
fn test_result_inside_for_loop_does_not_leave_loop_for_caller_next() {
    // The bare NEXT on line 30 must see the loop of line 10, not the body loop
    let (result, output) = run(&[
        "10 FOR I% = 1 TO 3",
        "20 PRINT FNfirst(I%)",
        "30 NEXT",
        "40 PRINT \"DONE\"",
        "50 END",
        "100 DEF FNfirst(N%)",
        "110 LOCAL J%",
        "120 FOR J% = 1 TO 10",
        "130 IF J% > N% THEN = J%",
        "140 NEXT J%",
        "150 = 0",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "2\n3\n4\nDONE\n");
}

#[test]
fn test_result_inside_repeat_loop_does_not_leave_loop_for_caller_until() {
    let (result, output) = run(&[
        "10 FOR I% = 1 TO 3",
        "20 R% = 0",
        "30 REPEAT",
        "40 R% = R% + 1",
        "50 PRINT FNcount(I%)",
        "60 UNTIL R% = 2",
        "70 NEXT",
        "80 PRINT \"DONE\"",
        "90 END",
        "100 DEF FNcount(N%)",
        "110 LOCAL K%",
        "120 REPEAT",
        "130 K% = K% + 1",
        "140 IF K% >= N% THEN = K% * 10",
        "150 UNTIL 0",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "10\n10\n20\n20\n30\n30\nDONE\n");
}

#[test]
fn test_result_inside_while_loop_does_not_leave_loop_for_caller_endwhile() {
    let (result, output) = run(&[
        "10 I% = 0",
        "20 WHILE I% < 3",
        "30 I% = I% + 1",
        "40 PRINT FNhalf(I% * 2)",
        "50 ENDWHILE",
        "60 PRINT \"DONE\"",
        "70 END",
        "100 DEF FNhalf(N%)",
        "110 LOCAL H%",
        "120 WHILE 1",
        "130 H% = H% + 1",
        "140 IF H% * 2 >= N% THEN = H%",
        "150 ENDWHILE",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n2\n3\nDONE\n");
}

#[test]
fn test_removed_function_is_unknown_on_next_run() {
    let mut executor = Executor::new();
    let mut program = ProgramStore::new();
    for line in [
        "10 A = FNa",
        "20 PRINT A",
        "30 END",
        "100 DEF FNa",
        "110 = 5",
    ] {
        program.store_line(tokenize(line).expect("line tokenizes"));
    }
    assert_eq!(run_program(&mut executor, &mut program), Ok(()));
    assert_eq!(executor.get_output(), "5\n");

    // Remove the DEF line, then run again with the same executor
    program.delete_line(100);
    let result = run_program(&mut executor, &mut program);

    assert_eq!(result, Err("No such FN/PROC: FNa at line 10".to_string()));
}

#[test]
fn test_single_line_function_in_immediate_mode_without_run() {
    let mut executor = Executor::new();
    for line in ["DEF FNsq(X) = X * X", "PRINT FNsq(3)"] {
        let statement =
            parse_statement(&tokenize(line).expect("line tokenizes")).expect("line parses");
        executor
            .execute_statement(&statement)
            .expect("statement runs");
    }

    assert_eq!(executor.get_output(), "9\n");
}

#[test]
fn test_single_line_function_defined_after_end() {
    let (result, output) = run(&["10 PRINT FNsq(4)", "20 END", "100 DEF FNsq(X) = X * X"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "16\n");
}
