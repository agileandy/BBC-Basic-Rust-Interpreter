//! Tests for functions that have more than one line (issue 6).

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;

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

    assert_eq!(
        result,
        Err("Runtime error at line 110: DivisionByZero".to_string())
    );
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
fn test_single_line_function_defined_after_end() {
    let (result, output) = run(&["10 PRINT FNsq(4)", "20 END", "100 DEF FNsq(X) = X * X"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "16\n");
}
