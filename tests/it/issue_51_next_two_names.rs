//! Issue 51: a NEXT with two or more loop names ends each loop in order.
//!
//! `NEXT J%, I%` first handles the loop of `J%`. If that loop continues,
//! the program jumps back to it. If it ends, the NEXT handles `I%` in the
//! same way.

use crate::common::run_timed_the_program_stops as run;

/// The output of the same program with one name in each NEXT.
fn reference(source: &[&str]) -> String {
    let (result, output) = run(source);
    assert_eq!(result, Ok(()));
    output
}

#[test]
fn test_two_names_on_one_line() {
    let (result, output) = run(&["10 FOR I%=1 TO 2 : FOR J%=1 TO 2 : PRINT I%;J% : NEXT J%, I%"]);

    assert_eq!(result, Ok(()));
    assert_eq!(
        output,
        reference(&["10 FOR I%=1 TO 2 : FOR J%=1 TO 2 : PRINT I%;J% : NEXT J% : NEXT I%"])
    );
    assert_eq!(output.lines().count(), 4);
}

#[test]
fn test_two_names_on_separate_lines() {
    let (result, output) = run(&[
        "10 FOR I%=1 TO 2",
        "20 FOR J%=1 TO 2",
        "30 PRINT I%;J%",
        "40 NEXT J%, I%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(
        output,
        reference(&[
            "10 FOR I%=1 TO 2",
            "20 FOR J%=1 TO 2",
            "30 PRINT I%;J%",
            "40 NEXT J%",
            "50 NEXT I%",
        ])
    );
    assert_eq!(output.lines().count(), 4);
}

#[test]
fn test_two_names_print_each_pair() {
    let (result, output) = run(&[
        "10 FOR I%=1 TO 2",
        "20 FOR J%=1 TO 2",
        "30 PRINT STR$(I%)+STR$(J%)",
        "40 NEXT J%, I%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "11\n12\n21\n22\n");
}

#[test]
fn test_three_names() {
    let (result, output) = run(&[
        "10 FOR I%=1 TO 2 : FOR J%=1 TO 2 : FOR K%=1 TO 2",
        "20 PRINT STR$(I%)+STR$(J%)+STR$(K%)",
        "30 NEXT K%, J%, I%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "111\n112\n121\n122\n211\n212\n221\n222\n");
}

#[test]
fn test_two_names_with_real_loop_variables() {
    let (result, output) = run(&[
        "10 FOR A=0.5 TO 1 STEP 0.5",
        "20 FOR B=1 TO 1.5 STEP 0.5",
        "30 PRINT A+B",
        "40 NEXT B, A",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1.5\n2\n2\n2.5\n");
}

#[test]
fn test_two_names_with_empty_inner_loop() {
    let (result, output) = run(&[
        "10 C%=0",
        "20 FOR I%=1 TO 3 : C%=C%+1 : FOR J%=1 TO 4 : NEXT J%, I%",
        "30 PRINT C%;\" \";I%;\" \";J%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "3 4 5\n");
}

#[test]
fn test_statements_after_next_run_once() {
    let (result, output) = run(&[
        "10 FOR I%=1 TO 2 : FOR J%=1 TO 2 : NEXT J%, I% : PRINT \"DONE\"",
        "20 PRINT \"END\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "DONE\nEND\n");
}

#[test]
fn test_second_name_with_no_open_loop_gives_next_error() {
    let (expected, _) = run(&["10 NEXT Z%"]);
    assert!(expected.is_err());

    let (result, output) = run(&["10 FOR J%=1 TO 2 : PRINT J% : NEXT J%, Z%"]);

    assert_eq!(result, expected);
    assert_eq!(output.lines().count(), 2);
}
