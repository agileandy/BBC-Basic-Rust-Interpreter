//! Issue #85: LIST shows a hex number as typed, and puts no space between a
//! function keyword and its opening bracket.
//!
//! Before the correction, `&1F` listed as `31`, and `VAL("1")` listed as
//! `VAL ("1")`. RENUMBER also changed `GOTO &46` into a decimal line number.

use crate::common::run_binary_read_first as run_binary;
use crate::common::run_lines as run;
use bbc_basic_interpreter::tokenizer::{detokenize, tokenize};

const PROGRAM: [&str; 7] = [
    "10 A% = &1F + &FFFFFFFF",
    "20 PRINT VAL(\"&1F\");LEN(\"AB\");LEFT$(\"XYZ\",1);FNa(A%)",
    "30 PRINT (1 + 2);TAB(5);A%;~&FF",
    "40 GOTO &46",
    "50 PRINT \"NO\"",
    "70 END",
    "80 DEF FNa(X)= X * 2",
];

/// The output of RUN before the correction
const RUN_OUTPUT: &str = "312X60\n3    30FF\n";

#[test]
fn test_list_shows_hex_numbers_and_function_brackets_as_typed() {
    let input = format!("{}\nLIST\nEXIT\n", PROGRAM.join("\n"));
    let output = run_binary(&input);

    for line in PROGRAM {
        assert!(output.contains(line), "{line:?} not in {output:?}");
    }
}

#[test]
fn test_run_output_is_unchanged() {
    let (result, output) = run(&PROGRAM);

    assert_eq!(result, Ok(()));
    assert_eq!(output, RUN_OUTPUT);
}

#[test]
fn test_listed_lines_tokenize_to_the_same_tokens_and_run_the_same() {
    let listed: Vec<String> = PROGRAM
        .iter()
        .map(|source| {
            let line = tokenize(source).expect("line tokenizes");
            let listed = detokenize(&line).expect("line detokenizes");
            assert_eq!(tokenize(&listed).expect("listed line tokenizes"), line);
            listed
        })
        .collect();
    let source: Vec<&str> = listed.iter().map(String::as_str).collect();

    let (result, output) = run(&source);

    assert_eq!(result, Ok(()));
    assert_eq!(output, RUN_OUTPUT);
}

#[test]
fn test_renumber_does_not_change_a_hex_goto_target() {
    let input = format!("{}\nRENUMBER\nLIST\nEXIT\n", PROGRAM.join("\n"));
    let output = run_binary(&input);

    assert!(output.contains("40 GOTO &46"), "output was {output:?}");
    assert!(!output.contains("GOTO 60"), "output was {output:?}");
}
