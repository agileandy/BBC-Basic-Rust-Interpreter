//! Issue #178: a comment that starts with an apostrophe keeps its text.
//! LIST shows the apostrophe and the text as typed, and a save and a load
//! keep the comment. An apostrophe after a statement ends the statement.

use crate::common::{run_in_dir, run_lines as run};
use bbc_basic_interpreter::tokenizer::{detokenize, tokenize};
use std::path::PathBuf;

/// The listed form of one line
fn listed(source: &str) -> String {
    detokenize(&tokenize(source).expect("line tokenizes")).expect("line lists")
}

/// A listed line tokenizes to the same tokens as the stored line (D2)
fn assert_same_tokens_after_list(source: &str) {
    let stored = tokenize(source).expect("line tokenizes");
    let again = tokenize(&listed(source)).expect("listed line tokenizes");
    assert_eq!(again, stored, "{:?}", source);
}

/// A directory for this test only
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bbc_issue_178_{}_{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir is created");
    dir
}

#[test]
fn test_comment_line_lists_as_typed_and_runs() {
    assert_eq!(listed("10 ' note"), "10 ' note");
    assert_same_tokens_after_list("10 ' note");
    let (result, output) = run(&["10 ' note", "20 PRINT \"A\""]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "A\n");
}

#[test]
fn test_comment_with_no_space_and_with_no_text_lists_as_typed() {
    assert_eq!(listed("10 'note"), "10 'note");
    assert_eq!(listed("10 '"), "10 '");
    assert_same_tokens_after_list("10 'note");
    assert_same_tokens_after_list("10 '");
}

#[test]
fn test_comment_after_a_colon_lists_as_typed_and_runs() {
    // LIST shows a colon with no spaces, as in `X = 1:REM a`
    assert_eq!(listed("10 X = 1:' note"), "10 X = 1:' note");
    assert_eq!(listed("10 X = 1 : ' note"), "10 X = 1:' note");
    assert_same_tokens_after_list("10 X = 1 : ' note");
    let (result, output) = run(&["10 X = 1 : ' note", "20 PRINT X"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n");
}

#[test]
fn test_comment_after_a_statement_lists_as_typed_and_ends_the_statement() {
    assert_eq!(listed("10 X = 1 ' note"), "10 X = 1 ' note");
    assert_same_tokens_after_list("10 X = 1 ' note");
    let (result, output) = run(&["10 X = 1 ' note", "20 PRINT X"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n");
}

#[test]
fn test_comment_text_is_kept_as_typed() {
    let source = "10 X = 1 ' a @ b, \"c\" : ELSE ' REM";
    assert_eq!(listed(source), source);
    assert_same_tokens_after_list(source);
    let (result, output) = run(&[source, "20 PRINT X"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n");
}

#[test]
fn test_comment_after_then_and_else_keeps_its_text() {
    let source = "10 IF X = 0 THEN Y = 1 ' yes ELSE Y = 2";
    assert_eq!(listed(source), source);
    let (result, output) = run(&["5 X = 0", source, "20 PRINT Y"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n");
    let source = "10 IF X = 1 THEN Y = 1 ELSE Y = 2 ' no";
    assert_eq!(listed(source), source);
    let (result, output) = run(&["5 X = 0", source, "20 PRINT Y"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "2\n");
}

#[test]
fn test_save_and_load_keep_the_comment() {
    let dir = temp_dir("save_load");
    let (_, output) = run_in_dir(
        &dir,
        &[
            "10 ' first",
            "20 X = 1 ' second",
            "30 X = X + 1:' third",
            "40 PRINT X",
            "SAVE \"comment.bas\"",
            "NEW",
            "LOAD \"comment.bas\"",
            "LIST",
            "RUN",
            "EXIT",
        ],
    );
    let saved = std::fs::read_to_string(dir.join("comment.bas")).expect("file is saved");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(saved.contains("10 ' first"), "{:?}", saved);
    assert!(saved.contains("20 X = 1 ' second"), "{:?}", saved);
    assert!(saved.contains("30 X = X + 1:' third"), "{:?}", saved);
    assert!(output.contains("10 ' first\n"), "{:?}", output);
    assert!(output.contains("20 X = 1 ' second\n"), "{:?}", output);
    assert!(output.contains("30 X = X + 1:' third\n"), "{:?}", output);
    assert!(output.contains("2\n"), "{:?}", output);
    assert!(!output.contains("rror"), "{:?}", output);
}

#[test]
fn test_apostrophe_in_print_is_unchanged() {
    assert_eq!(listed("10 PRINT \"A\"'\"B\""), "10 PRINT \"A\"'\"B\"");
    let (result, output) = run(&["10 PRINT \"A\"'\"B\""]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "A\nB\n");
}

#[test]
fn test_apostrophe_in_a_string_is_unchanged() {
    assert_eq!(listed("10 PRINT \"it's\""), "10 PRINT \"it's\"");
    let (result, output) = run(&["10 PRINT \"it's\""]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "it's\n");
}

#[test]
fn test_apostrophe_in_a_rem_is_part_of_the_rem_text() {
    assert_eq!(listed("10 REM it's here"), "10 REM it's here");
    assert_same_tokens_after_list("10 REM it's here");
    let (result, output) = run(&["10 REM it's here", "20 PRINT \"A\""]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "A\n");
}

#[test]
fn test_apostrophe_in_data_is_unchanged() {
    assert_eq!(listed("10 DATA it's, 2"), "10 DATA it's, 2");
    let (result, output) = run(&["10 DATA it's, 2", "20 READ A$, B", "30 PRINT A$;B"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "it's2\n");
}
