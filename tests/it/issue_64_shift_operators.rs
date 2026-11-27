//! Issue #64: a program can write the shift operators `<<` and `>>`.

use crate::common::run_lines as run;
use bbc_basic_interpreter::tokenizer::{detokenize, tokenize};

fn assert_prints(source: &[&str], expected: &str) {
    let (result, output) = run(source);
    assert_eq!(result, Ok(()), "{source:?}");
    assert_eq!(output, expected, "{source:?}");
}

#[test]
fn test_left_shift() {
    assert_prints(&["10 PRINT 1 << 4"], "16\n");
}

#[test]
fn test_right_shift() {
    assert_prints(&["10 PRINT 256 >> 4"], "16\n");
}

#[test]
fn test_right_shift_keeps_the_sign() {
    assert_prints(&["10 PRINT -8 >> 1"], "-4\n");
}

#[test]
fn test_left_shift_of_40_places_gives_zero() {
    assert_prints(&["10 PRINT 1 << 40"], "0\n");
}

#[test]
fn test_shift_in_an_if_condition() {
    assert_prints(&["10 IF 1 << 2 = 4 THEN PRINT \"OK\""], "OK\n");
}

#[test]
fn test_comparisons_keep_their_behaviour() {
    assert_prints(
        &[
            "10 A% = 1",
            "20 B% = 2",
            "30 IF A% < B% THEN PRINT \"LT\"",
            "40 IF A% <= B% THEN PRINT \"LE\"",
            "50 IF A% <> B% THEN PRINT \"NE\"",
            "60 IF B% > A% THEN PRINT \"GT\"",
            "70 IF B% >= A% THEN PRINT \"GE\"",
        ],
        "LT\nLE\nNE\nGT\nGE\n",
    );
}

#[test]
fn test_unsigned_right_shift_gives_error() {
    let (result, output) = run(&["10 PRINT 1 >>> 2"]);
    assert!(result.is_err(), "{result:?}");
    assert_eq!(output, "");
}

#[test]
fn test_detokenize_shows_shift_operators_as_typed() {
    let line = tokenize("10 PRINT 1 << 4 >> 2").expect("line tokenizes");
    let text = detokenize(&line).expect("line detokenizes");
    assert!(text.contains("<<"), "{text}");
    assert!(text.contains(">>"), "{text}");
}
