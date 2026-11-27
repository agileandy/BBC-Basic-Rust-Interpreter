//! Issue #158: NOT gives the bit complement of the 32-bit integer value of its
//! operand, as on the BBC Micro. NOT 4 is -5, and NOT TRUE is still FALSE.

use crate::common::run_with_limit as run;

/// Run the program and give its output lines. Panic if the program fails.
fn lines_of(source: &[&str]) -> Vec<String> {
    let (result, output) = run(source);
    assert_eq!(result, Ok(()), "output: {}", output);
    output.lines().map(|line| line.trim().to_string()).collect()
}

#[test]
fn not_of_a_whole_number_is_its_bit_complement() {
    assert_eq!(
        lines_of(&[
            "10 PRINT NOT 4",
            "20 PRINT 13 AND NOT 4",
            "30 PRINT NOT 0",
            "40 PRINT NOT -1",
        ]),
        vec!["-5", "9", "-1", "0"]
    );
}

#[test]
fn not_true_and_not_false_do_not_change() {
    assert_eq!(
        lines_of(&["10 PRINT NOT TRUE", "20 PRINT NOT FALSE"]),
        vec!["0", "-1"]
    );
}

#[test]
fn not_truncates_a_real_operand() {
    assert_eq!(
        lines_of(&["10 PRINT NOT 2.9", "20 A = 4.5", "30 PRINT NOT A"]),
        vec!["-3", "-5"]
    );
}

#[test]
fn not_in_a_real_context_gives_the_bit_complement() {
    assert_eq!(
        lines_of(&["10 PRINT 0.5 + NOT 4", "20 PRINT 0.5 + NOT 2.9"]),
        vec!["-4.5", "-2.5"]
    );
}

#[test]
fn not_of_an_integer_variable_is_its_bit_complement() {
    assert_eq!(
        lines_of(&["10 A% = &FF", "20 B% = NOT A%", "30 PRINT ~B%"]),
        vec!["FFFFFF00"]
    );
}

#[test]
fn not_of_a_value_out_of_range_is_too_big() {
    let (result, _) = run(&["10 PRINT NOT 3E10"]);
    assert_eq!(result, Err("Too big at line 10".to_string()));
}

#[test]
fn if_not_of_a_bit_test_uses_the_bit_complement() {
    // A% = 4: A% AND 4 is 4, so NOT 4 is -5 (true).
    // A% = 3: A% AND 4 is 0, so NOT 0 is -1 (true).
    assert_eq!(
        lines_of(&[
            "10 A% = 4",
            "20 IF NOT (A% AND 4) THEN PRINT \"yes\" ELSE PRINT \"no\"",
            "30 A% = 3",
            "40 IF NOT (A% AND 4) THEN PRINT \"yes\" ELSE PRINT \"no\"",
            "50 IF (A% AND 4) = 0 THEN PRINT \"clear\"",
        ]),
        vec!["yes", "yes", "clear"]
    );
}

#[test]
fn repeat_until_not_flag_stops_when_the_flag_is_false() {
    assert_eq!(
        lines_of(&[
            "10 F% = TRUE",
            "20 C% = 0",
            "30 REPEAT",
            "40 C% = C% + 1",
            "50 IF C% = 3 THEN F% = FALSE",
            "60 UNTIL NOT F%",
            "70 PRINT C%",
        ]),
        vec!["3"]
    );
}
