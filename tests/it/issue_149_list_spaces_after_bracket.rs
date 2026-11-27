//! Issue #149: LIST puts the same spaces on the 2 sides of an operator after
//! a closing bracket.
//!
//! Before the correction, `PRINT LEN(A$)+ B$` listed with no space before the
//! plus sign and one space after it. A unary minus before a name also listed
//! with a space after it: `ABS(- X)`.

use crate::common::run_binary_read_first as run_binary;
use bbc_basic_interpreter::tokenizer::{detokenize, tokenize};

/// Each line lists as it is typed here
const PROGRAM: [&str; 13] = [
    "10 PRINT LEN(A$) + B$",
    "20 X = (1 + 2) * 3",
    "30 IF (A) = 1 THEN PRINT \"Y\"",
    "40 PRINT (A) <> 1;(A) <= 2;(A) >= 3",
    "50 PRINT (-1)",
    "60 PRINT 2 * -1",
    "70 PRINT ABS(-X)",
    "80 X = -Y",
    "90 PRINT 2 * -Y,(-Y)",
    "100 A%(1)?2 = 3",
    "110 PRINT LEN(A$) - 1",
    "120 PRINT (A) AND 1",
    "130 CASE LEN(A$) OF",
];

fn listed(source: &str) -> String {
    let line = tokenize(source).expect("line tokenizes");
    let listed = detokenize(&line).expect("line detokenizes");
    assert_eq!(
        tokenize(&listed).expect("listed line tokenizes"),
        line,
        "{listed:?} does not give the tokens of {source:?}"
    );
    listed
}

#[test]
fn test_each_line_lists_as_typed_and_gives_the_same_tokens() {
    for source in PROGRAM {
        assert_eq!(listed(source), source);
    }
}

#[test]
fn test_uneven_spaces_after_a_bracket_list_evenly() {
    assert_eq!(
        listed("10 PRINT LEN(A$)+ B$ : X=1+2"),
        "10 PRINT LEN(A$) + B$:X = 1 + 2"
    );
    assert_eq!(listed("20 X=(1+2)*3"), "20 X = (1 + 2) * 3");
    assert_eq!(listed("30 IF (A)=1 THEN"), "30 IF (A) = 1 THEN");
}

#[test]
fn test_spaces_round_colon_comma_and_semicolon_do_not_change() {
    assert_eq!(listed("10 PRINT A, B; C : D = 1"), "10 PRINT A,B;C:D = 1");
    assert_eq!(
        listed("20 PRINT (A),(B);(C):D = 1"),
        "20 PRINT (A),(B);(C):D = 1"
    );
}

#[test]
fn test_list_command_shows_even_spaces_after_a_bracket() {
    let input = format!("{}\nLIST\nEXIT\n", PROGRAM.join("\n"));
    let output = run_binary(&input);

    for line in PROGRAM {
        assert!(output.contains(line), "{line:?} not in {output:?}");
    }
}
