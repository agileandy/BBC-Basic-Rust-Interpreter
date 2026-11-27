//! Issue #131: `TAB(x, y)` in PRINT moves the text cursor as `VDU 31, x, y`
//! does, and `PRINT#` does not permit TAB or SPC.

use crate::common::run_binary_read_first as run_binary;
use crate::common::run_timed_ran_for as run;
use bbc_basic_interpreter::tokenizer::{detokenize, tokenize};

#[test]
fn test_tab_with_2_values_sets_the_column_and_sends_no_escape_code() {
    let (result, output) = run(&["10 PRINT TAB(5, 3); \"X\";", "20 PRINT POS"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "X6\n");
    assert!(!output.contains('\x1b'), "output: {output:?}");
}

#[test]
fn test_tab_with_2_values_has_the_same_effect_as_vdu_31() {
    let (result, output) = run(&[
        "10 PRINT \"ABCDEFGH\"; TAB(2, 0); \"X\"; : PRINT POS",
        "20 VDU 31, 2, 0 : PRINT \"X\"; : PRINT POS",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "ABCDEFGHX3\nX3\n");
}

#[test]
fn test_tab_with_2_values_takes_expressions_and_runs_each_one_time() {
    let (result, output) = run(&[
        "10 X% = 3",
        "20 PRINT TAB(X% + FNa, FNa * 2); \"X\"; : PRINT POS; C%",
        "30 END",
        "200 DEF FNa",
        "210 C% = C% + 1",
        "220 = 1",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "X52\n");
}

#[test]
fn test_tab_with_2_values_truncates_a_real() {
    let (result, output) = run(&["10 PRINT TAB(4.7, 1.2); \"X\"; : PRINT POS"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "X5\n");
}

#[test]
fn test_tab_with_1_value_does_not_change() {
    let (result, output) = run(&["10 PRINT \"AB\"; TAB(5); \"X\"; : PRINT POS"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "AB   X6\n");
}

#[test]
fn test_tab_with_3_values_is_a_syntax_error() {
    let (result, _) = run(&["10 PRINT TAB(1, 2, 3); \"X\""]);

    assert_eq!(
        result,
        Err("Syntax error at line 10: TAB takes 1 or 2 values".to_string())
    );
}

#[test]
fn test_print_file_with_tab_is_a_syntax_error() {
    let (result, _) = run(&["10 PRINT#1, TAB(3); \"X\""]);

    assert_eq!(
        result,
        Err("Syntax error at line 10: TAB and SPC are not permitted in PRINT#".to_string())
    );
}

#[test]
fn test_print_file_with_spc_is_a_syntax_error() {
    let (result, _) = run(&["10 PRINT#1, \"A\", SPC(2)"]);

    assert_eq!(
        result,
        Err("Syntax error at line 10: TAB and SPC are not permitted in PRINT#".to_string())
    );
}

// The tokenizer does not keep the spaces next to `,` and `;` in any
// statement, so LIST shows `TAB(5,3);` as it shows `TAB(5);` and `VDU 31,4,10`.

#[test]
fn test_listed_tab_with_2_values_tokenizes_to_the_same_tokens() {
    let line = tokenize("10 PRINT TAB(5, 3); \"X\"").expect("line tokenizes");
    let listed = detokenize(&line).expect("line detokenizes");

    assert_eq!(listed, "10 PRINT TAB(5,3);\"X\"");
    assert_eq!(tokenize(&listed).expect("listed line tokenizes"), line);
}

#[test]
fn test_list_shows_tab_with_2_values() {
    let output = run_binary("10 PRINT TAB(5, 3); \"X\"\nLIST\nEXIT\n");

    assert!(
        output.contains("10 PRINT TAB(5,3);\"X\"\n"),
        "output: {output:?}"
    );
}

#[test]
fn test_listed_tab_with_2_values_runs_as_typed() {
    let (result, output) = run(&["10 PRINT TAB(5,3);\"X\";:PRINT POS"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "X6\n");
}
