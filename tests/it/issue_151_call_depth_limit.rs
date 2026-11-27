//! Issue 151: a call that repeats without end gives "No room".
//!
//! Each program that can overflow the native stack on a defect runs in the
//! real binary, so that an overflow cannot end the test run.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;

use crate::common::run_binary_read_first;

/// The ERR number of "No room".
const NO_ROOM_ERR: &str = "11";

/// Run the program lines in the binary, then RUN, a PRINT that shows the
/// prompt is back, and EXIT. Give stdout.
fn run_then_print(lines: &[&str]) -> String {
    let mut input = String::new();
    for line in lines {
        input.push_str(line);
        input.push('\n');
    }
    input.push_str("RUN\nPRINT \"back\"\nEXIT\n");
    run_binary_read_first(&input)
}

/// Assert that the run gave `No room at line N` and that the prompt came back.
fn assert_no_room_then_back(output: &str, line: u32) {
    assert!(
        output.contains(&format!("Error: No room at line {line}\n")),
        "output: {output}"
    );
    assert!(output.contains("back\n"), "output: {output}");
}

#[test]
fn test_single_line_function_without_end_gives_no_room() {
    let output = run_then_print(&["10 PRINT FNr(1)", "30 DEF FNr(N) = FNr(N + 1)"]);
    assert_no_room_then_back(&output, 30);
}

#[test]
fn test_multi_line_function_without_end_gives_no_room() {
    let output = run_then_print(&["10 PRINT FNr(1)", "30 DEF FNr(N)", "40 = FNr(N + 1)"]);
    assert_no_room_then_back(&output, 40);
}

/// A statement in the function body uses the most native stack for each call
#[test]
fn test_function_that_calls_itself_in_a_statement_gives_no_room() {
    let output = run_then_print(&[
        "10 X = FNr(1)",
        "30 DEF FNr(N)",
        "40 IF N > 0 THEN X = FNr(N + 1)",
        "50 = 0",
    ]);
    assert_no_room_then_back(&output, 40);
}

#[test]
fn test_function_that_calls_itself_through_a_procedure_gives_no_room() {
    let output = run_then_print(&[
        "10 X = FNr(1)",
        "30 DEF FNr(N)",
        "40 PROCp(N)",
        "50 = 0",
        "60 DEF PROCp(N)",
        "70 X = FNr(N + 1)",
        "80 ENDPROC",
    ]);
    assert_no_room_then_back(&output, 70);
}

#[test]
fn test_eval_of_itself_gives_no_room() {
    let output = run_then_print(&["10 A$ = \"EVAL(A$)\"", "20 PRINT EVAL(A$)"]);
    assert_no_room_then_back(&output, 20);
}

#[test]
fn test_procedure_without_end_gives_no_room() {
    let output = run_then_print(&[
        "10 PROCr(1)",
        "30 DEF PROCr(N)",
        "40 PROCr(N + 1)",
        "50 ENDPROC",
    ]);
    assert_no_room_then_back(&output, 40);
}

#[test]
fn test_gosub_without_end_gives_no_room() {
    let output = run_then_print(&["10 GOSUB 10"]);
    assert_no_room_then_back(&output, 10);
}

/// The result, the output and the FOR, REPEAT and WHILE stack depths at
/// the end of a run
type RunWithDepths = (Result<(), String>, String, (usize, usize, usize));

/// Run the program in the library on a thread with a time limit of 5
/// seconds. Give the result, the output and the loop stack depths at the
/// end. Use it only for a program with no deep calls.
fn run_with_depths(source: &[String]) -> RunWithDepths {
    let source = source.to_vec();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut executor = Executor::new();
        let mut program = ProgramStore::new();
        for line in &source {
            program.store_line(tokenize(line).expect("line tokenizes"));
        }
        let result = run_program(&mut executor, &mut program);
        let depths = executor.loop_stack_depths();
        let _ = sender.send((result, executor.get_output().to_string(), depths));
    });
    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("the program stops in 5 seconds")
}

/// `run_with_depths` for program lines given as text
fn run_lines_with_depths(source: &[&str]) -> RunWithDepths {
    let lines: Vec<String> = source.iter().map(|line| line.to_string()).collect();
    run_with_depths(&lines)
}

#[test]
fn test_for_started_again_by_goto_uses_one_entry() {
    let (result, _, depths) = run_lines_with_depths(&[
        "10 FOR I = 1 TO 2",
        "20 C% = C% + 1 : IF C% < 100 THEN GOTO 10",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(depths, (1, 0, 0));
}

#[test]
fn test_for_started_again_removes_the_loops_above_it() {
    let (result, _, depths) = run_lines_with_depths(&[
        "10 FOR I = 1 TO 2",
        "20 FOR J = 1 TO 2",
        "30 C% = C% + 1 : IF C% < 100 THEN GOTO 10",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(depths, (2, 0, 0));
}

#[test]
fn test_for_each_started_again_by_goto_uses_one_entry() {
    let (result, _, depths) = run_lines_with_depths(&[
        "10 L = NEWLIST : LISTADD L, 1 : LISTADD L, 2",
        "20 FOR EACH N IN L",
        "30 C% = C% + 1 : IF C% < 100 THEN GOTO 20",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(depths, (1, 0, 0));
}

#[test]
fn test_repeat_started_again_by_goto_uses_one_entry() {
    let (result, _, depths) =
        run_lines_with_depths(&["10 REPEAT", "20 C% = C% + 1 : IF C% < 100 THEN GOTO 10"]);
    assert_eq!(result, Ok(()));
    assert_eq!(depths, (0, 1, 0));
}

#[test]
fn test_while_started_again_by_goto_uses_one_entry() {
    let (result, _, depths) =
        run_lines_with_depths(&["10 WHILE TRUE", "20 C% = C% + 1 : IF C% < 100 THEN GOTO 10"]);
    assert_eq!(result, Ok(()));
    assert_eq!(depths, (0, 0, 1));
}

#[test]
fn test_while_started_again_with_a_false_condition_closes_its_entry() {
    let (result, output, depths) = run_lines_with_depths(&[
        "10 WHILE C% < 100",
        "20 C% = C% + 1 : GOTO 10",
        "30 ENDWHILE",
        "40 PRINT C%",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "       100\n");
    assert_eq!(depths, (0, 0, 0));
}

#[test]
fn test_long_program_that_leaves_a_loop_with_goto_completes() {
    let (result, output, depths) = run_lines_with_depths(&[
        "20 FOR I = 1 TO 5",
        "30 IF I = 2 THEN GOTO 50",
        "40 NEXT",
        "50 C% = C% + 1",
        "60 IF C% < 20000 THEN GOTO 20",
        "70 PRINT C%",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "     20000\n");
    assert_eq!(depths, (1, 0, 0));
}

#[test]
fn test_for_in_a_procedure_keeps_the_loop_of_the_caller() {
    let (result, output, _) = run_lines_with_depths(&[
        "10 FOR I = 1 TO 2",
        "20 PROCp",
        "30 NEXT",
        "40 PRINT \"done\"",
        "50 END",
        "60 DEF PROCp",
        "70 LOCAL I",
        "80 FOR I = 1 TO 2",
        "90 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "done\n");
}

#[test]
fn test_for_in_a_function_keeps_the_loop_of_the_caller() {
    let (result, output, _) = run_lines_with_depths(&[
        "10 FOR I = 1 TO 2",
        "20 X = FNf",
        "30 NEXT",
        "40 PRINT \"done\"",
        "50 END",
        "60 DEF FNf",
        "70 LOCAL I",
        "80 FOR I = 1 TO 2",
        "90 = 0",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "done\n");
}

#[test]
fn test_for_at_the_stack_limit_changes_nothing() {
    let mut lines = vec!["1 ON ERROR GOTO 30000".to_string()];
    for number in 1..=10_000 {
        lines.push(format!("{} FOR V{number}% = 1 TO 2", number + 1));
    }
    lines.push("20000 Z = 3 : FOR Z = 5 TO 6".to_string());
    lines.push("30000 PRINT ERR; \" \"; ERL; \" \"; Z".to_string());
    let (result, output, _) = run_with_depths(&lines);
    assert_eq!(result, Ok(()));
    assert_eq!(output, format!("        {NO_ROOM_ERR} 20000 3\n"));
}

#[test]
fn test_str_of_an_error_gives_that_error() {
    let (result, _, _) = run_lines_with_depths(&["10 PRINT STR$(1 / 0)"]);
    assert_eq!(result, Err("Division by zero at line 10".to_string()));
}

/// STR$ must not try a second evaluator after No room: that doubles the
/// calls at each level
#[test]
fn test_function_inside_str_and_val_gives_no_room() {
    let output = run_then_print(&["10 PRINT FNr(1)", "20 DEF FNr(N) = VAL(STR$(FNr(N + 1)))"]);
    assert_no_room_then_back(&output, 20);
}

#[test]
fn test_string_function_inside_val_and_str_gives_no_room() {
    let output = run_then_print(&[
        "10 PRINT FNr$(1)",
        "20 DEF FNr$(N) = STR$(VAL(FNr$(N + 1)))",
    ]);
    assert_no_room_then_back(&output, 20);
}

#[test]
fn test_function_inside_nested_numeric_builtins_gives_no_room() {
    let output = run_then_print(&[
        "10 PRINT FNr(1)",
        "20 DEF FNr(N) = ABS(INT(ABS(INT(ABS(INT(ABS(INT(FNr(N + 1)))))))))",
    ]);
    assert_no_room_then_back(&output, 20);
}

#[test]
fn test_function_inside_nested_string_builtins_gives_no_room() {
    let output = run_then_print(&[
        "10 PRINT FNr$(1)",
        "20 DEF FNr$(N) = LEFT$(MID$(LEFT$(MID$(FNr$(N + 1), 1), 5), 1), 5)",
    ]);
    assert_no_room_then_back(&output, 20);
}

#[test]
fn test_eval_of_a_deep_bracket_nest_gives_no_room() {
    let output = run_then_print(&[
        "10 A$ = STRING$(20000, \"(\") + \"1\" + STRING$(20000, \")\")",
        "20 PRINT EVAL(A$)",
    ]);
    assert_no_room_then_back(&output, 20);
}

#[test]
fn test_stored_line_with_a_deep_bracket_nest_gives_no_room() {
    let line = format!("10 PRINT {}1{}", "(".repeat(20_000), ")".repeat(20_000));
    let output = run_then_print(&[&line]);
    assert!(output.contains("No room"), "output: {output}");
    assert!(output.contains("back\n"), "output: {output}");
}

#[test]
fn test_typed_line_with_a_deep_bracket_nest_gives_no_room() {
    let line = format!("PRINT {}1{}", "(".repeat(20_000), ")".repeat(20_000));
    let output = run_binary_read_first(&format!("{line}\nPRINT \"back\"\nEXIT\n"));
    assert!(output.contains("No room"), "output: {output}");
    assert!(output.contains("back\n"), "output: {output}");
}

/// The parser allows a nest of 500 levels: N brackets use N + 1 levels
#[test]
fn test_nests_below_the_parser_limit_work() {
    let output = run_then_print(&[
        "10 PRINT EVAL(STRING$(498, \"(\") + \"1\" + STRING$(498, \")\"))",
        "20 PRINT EVAL(STRING$(499, \"-\") + \"1\")",
        "30 PRINT EVAL(STRING$(240, \"ABS(\") + \"-2\" + STRING$(240, \")\"))",
    ]);
    assert!(
        output.contains("         1\n        -1\n         2\n"),
        "output: {output}"
    );
    assert!(output.contains("back\n"), "output: {output}");
}

#[test]
fn test_nest_above_the_parser_limit_gives_no_room() {
    let output = run_then_print(&[
        "10 ON ERROR PRINT \"ERR \"; ERR : END",
        "20 PRINT EVAL(STRING$(500, \"(\") + \"1\" + STRING$(500, \")\"))",
    ]);
    assert!(
        output.contains(&format!("ERR {NO_ROOM_ERR}\n")),
        "output: {output}"
    );
    assert!(output.contains("back\n"), "output: {output}");
}

#[test]
fn test_bracket_nest_of_depth_100_works() {
    let output = run_then_print(&[
        "10 A$ = STRING$(100, \"(\") + \"1\" + STRING$(100, \")\")",
        "20 PRINT EVAL(A$) + ((((((((((2))))))))))",
    ]);
    assert!(output.contains("3\n"), "output: {output}");
    assert!(output.contains("back\n"), "output: {output}");
}

#[test]
fn test_recursive_function_of_depth_200_works() {
    let output = run_then_print(&[
        "10 PRINT FNsum(200)",
        "20 PRINT FNfact(10)",
        "30 END",
        "40 DEF FNsum(N)",
        "50 IF N = 0 THEN = 0",
        "60 = N + FNsum(N - 1)",
        "70 DEF FNfact(N)",
        "80 IF N <= 1 THEN = 1",
        "90 = N * FNfact(N - 1)",
    ]);
    assert!(
        output.contains("     20100\n   3628800\n"),
        "output: {output}"
    );
    assert!(output.contains("back\n"), "output: {output}");
}

#[test]
fn test_recursive_procedure_of_depth_2000_works() {
    let output = run_then_print(&[
        "10 C% = 0",
        "20 PROCwalk(2000)",
        "30 PRINT C%",
        "40 END",
        "50 DEF PROCwalk(N)",
        "60 LOCAL X",
        "70 X = N",
        "80 C% = C% + 1",
        "90 IF N > 1 THEN PROCwalk(N - 1)",
        "100 IF X <> N THEN PRINT \"bad local\"",
        "110 ENDPROC",
    ]);
    assert!(output.contains("2000\n"), "output: {output}");
    assert!(!output.contains("bad local"), "output: {output}");
    assert!(output.contains("back\n"), "output: {output}");
}

#[test]
fn test_handler_traps_no_room_and_a_later_call_works() {
    let output = run_then_print(&[
        "10 ON ERROR GOTO 100",
        "20 PRINT FNr(1)",
        "30 END",
        "40 DEF FNr(N) = FNr(N + 1)",
        "50 DEF FNsq(N) = N * N",
        "100 PRINT \"ERR \"; ERR; \" ERL \"; ERL",
        "110 PRINT FNsq(7)",
        "120 PRINT EVAL(\"2 + 3\")",
    ]);
    assert!(
        output.contains(&format!(
            "ERR {NO_ROOM_ERR} ERL 40\n        49\n         5\n"
        )),
        "output: {output}"
    );
    assert!(output.contains("back\n"), "output: {output}");
}

#[test]
fn test_handler_traps_no_room_of_a_procedure_and_a_later_call_works() {
    let output = run_then_print(&[
        "10 ON ERROR GOTO 100",
        "20 PROCr",
        "30 END",
        "40 DEF PROCr",
        "50 PROCr",
        "60 ENDPROC",
        "70 DEF PROCok",
        "80 PRINT \"ok\"",
        "90 ENDPROC",
        "100 PRINT \"ERR \"; ERR",
        "110 PROCok",
    ]);
    assert!(
        output.contains(&format!("ERR {NO_ROOM_ERR}\nok\n")),
        "output: {output}"
    );
    assert!(output.contains("back\n"), "output: {output}");
}
