//! Issue #69: LIST shows the operators `<=`, `>=` and `<>` as typed.
//!
//! Before the correction, LIST put a space between the 2 characters, for
//! example `< =`. The listed line must also tokenize to the same tokens.

use crate::common::run_binary_read_first as run_binary;
use crate::common::run_lines as run;
use bbc_basic_interpreter::tokenizer::{detokenize, tokenize};

const PROGRAM: [&str; 3] = [
    "10 IF A% <= B% THEN PRINT \"LE\"",
    "20 IF A% >= B% THEN PRINT \"GE\"",
    "30 IF A% <> B% THEN PRINT \"NE\"",
];

#[test]
fn test_list_shows_comparison_operators_as_typed() {
    let input = format!("{}\nLIST\nEXIT\n", PROGRAM.join("\n"));
    let output = run_binary(&input);

    for line in PROGRAM {
        assert!(output.contains(line), "output was {output:?}");
    }
    for split in ["< =", "> =", "< >"] {
        assert!(!output.contains(split), "output was {output:?}");
    }
}

#[test]
fn test_listed_lines_tokenize_to_the_same_tokens() {
    for source in PROGRAM {
        let line = tokenize(source).expect("line tokenizes");
        let listed = detokenize(&line).expect("line detokenizes");
        assert_eq!(listed, source);
        assert_eq!(tokenize(&listed).expect("listed line tokenizes"), line);
    }
}

#[test]
fn test_listed_lines_run_as_typed() {
    let listed: Vec<String> = PROGRAM
        .iter()
        .map(|source| detokenize(&tokenize(source).expect("line tokenizes")).expect("detokenizes"))
        .collect();
    let mut source = vec!["5 A% = 1 : B% = 2"];
    source.extend(listed.iter().map(String::as_str));

    let (result, output) = run(&source);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "LE\nNE\n");
}
