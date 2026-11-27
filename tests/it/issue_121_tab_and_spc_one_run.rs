//! Tests that TAB and SPC in PRINT evaluate their argument one time, also
//! when the argument gives an error (issue #121).

use crate::common::run_timed_ran_for as run;

/// Run `print_line` (line 20) with an ON ERROR handler that prints `C%`.
/// `FNa` adds 1 to `C%` and then divides by zero.
fn counter_after_error(print_line: &str) -> String {
    let source = [
        "10 ON ERROR GOTO 100",
        print_line,
        "90 END",
        "100 PRINT C%",
        "110 END",
        "200 DEF FNa",
        "210 C% = C% + 1",
        "220 = 1 / 0",
    ];
    let (result, output) = run(&source);
    assert_eq!(result, Ok(()), "the handler runs and the program ends");
    output
}

#[test]
fn test_tab_runs_its_function_one_time_on_error() {
    assert_eq!(counter_after_error("20 PRINT TAB(FNa)"), "         1\n");
}

#[test]
fn test_spc_runs_its_function_one_time_on_error() {
    assert_eq!(counter_after_error("20 PRINT SPC(FNa)"), "         1\n");
}

#[test]
fn test_tab_function_error_text_with_no_handler() {
    let (result, _) = run(&["10 PRINT TAB(FNa)", "20 END", "200 DEF FNa", "210 = 1 / 0"]);
    assert_eq!(result, Err("Division by zero at line 210".to_string()));
}

#[test]
fn test_tab_with_integer_and_real_values() {
    let (result, output) = run(&[
        "10 PRINT TAB(3); \"A\"",
        "20 PRINT TAB(2.7); \"B\"",
        "30 X = 4.5",
        "40 PRINT TAB(X); \"C\"",
        "50 N% = 1",
        "60 PRINT TAB(N% + 1); \"D\"",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "   A\n  B\n    C\n  D\n");
}

#[test]
fn test_spc_with_integer_and_real_values() {
    let (result, output) = run(&[
        "10 PRINT \"A\"; SPC(3); \"B\"",
        "20 PRINT \"C\"; SPC(2.9); \"D\"",
        "30 X = 1.5",
        "40 PRINT \"E\"; SPC(X); \"F\"",
        "50 PRINT \"G\"; SPC(FNb); \"H\"",
        "60 END",
        "200 DEF FNb",
        "210 = 2",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "A   B\nC  D\nE F\nG  H\n");
}
