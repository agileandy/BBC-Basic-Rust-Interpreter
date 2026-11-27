//! Issue 151: a call that repeats without end gives "No room".
//!
//! Each program that can overflow the native stack on a defect runs in the
//! real binary, so that an overflow cannot end the test run.

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
    assert_no_room_then_back(&output, 10);
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

#[test]
fn test_for_started_again_by_goto_gives_no_room() {
    let output = run_then_print(&["10 FOR I = 1 TO 2", "20 GOTO 10"]);
    assert_no_room_then_back(&output, 10);
}

#[test]
fn test_repeat_started_again_by_goto_gives_no_room() {
    let output = run_then_print(&["10 REPEAT", "20 GOTO 10"]);
    assert_no_room_then_back(&output, 10);
}

#[test]
fn test_while_started_again_by_goto_gives_no_room() {
    let output = run_then_print(&["10 WHILE TRUE", "20 GOTO 10"]);
    assert_no_room_then_back(&output, 10);
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
    assert!(output.contains("20100\n3628800\n"), "output: {output}");
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
        output.contains(&format!("ERR {NO_ROOM_ERR} ERL 20\n49\n5\n")),
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
