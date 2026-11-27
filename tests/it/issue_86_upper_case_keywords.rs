//! Issue #86: a keyword matches only in upper case, as in BBC BASIC.
//!
//! Before the correction, the tokenizer changed each word to upper case
//! before the keyword lookup. So a variable such as `count` or `to` gave a
//! parse error. The typed commands of the command loop, such as `list` and
//! `run`, still work in any letter case.

use crate::common::run_binary_read_first as run_binary;
use crate::common::run_lines as run;

#[test]
fn test_lower_case_count_is_a_variable() {
    let (result, output) = run(&["10 count = 5 : PRINT count"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "5\n");
}

#[test]
fn test_for_loop_with_lower_case_variable_runs() {
    let (result, output) = run(&["10 FOR i = 1 TO 3 : PRINT i : NEXT i"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n2\n3\n");
}

#[test]
fn test_lower_case_time_to_and_step_are_variables() {
    let (result, output) = run(&["10 time = 1 : to = 2 : step = 3 : PRINT time + to + step"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "6\n");
}

#[test]
fn test_lower_case_pi_val_len_and_endwhile_are_variables() {
    let (result, output) =
        run(&["10 pi = 1 : val = 2 : len = 3 : endwhile = 4 : PRINT pi + val + len + endwhile"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "10\n");
}

#[test]
fn test_lower_case_print_is_not_a_keyword() {
    let (result, output) = run(&["10 print 1"]);

    assert!(result.is_err(), "result was {:?}", result);
    assert_eq!(output, "");
}

#[test]
fn test_typed_commands_work_in_lower_case() {
    let output = run_binary("10 count = 5 : PRINT count\nlist\nrun\nEXIT\n");

    assert!(
        output.contains("> 10 count = 5:PRINT count\n"),
        "output was {:?}",
        output
    );
    assert!(output.contains("> 5\n"), "output was {:?}", output);
}
