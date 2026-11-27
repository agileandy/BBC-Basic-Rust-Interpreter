//! Issue #179: the text after REM is kept as typed. LIST shows REM and the
//! text with each character and space as typed, and a save and a load keep
//! the text. A REM ends its line, so a colon after REM is part of the comment.

use crate::common::{run_in_dir, run_lines as run};
use bbc_basic_interpreter::tokenizer::{detokenize, tokenize};
use std::path::PathBuf;

/// The listed form of one line
fn listed(source: &str) -> String {
    detokenize(&tokenize(source).expect("line tokenizes")).expect("line lists")
}

/// A listed line tokenizes to the same tokens as the stored line (D4)
fn assert_same_tokens_after_list(source: &str) {
    let stored = tokenize(source).expect("line tokenizes");
    let again = tokenize(&listed(source)).expect("listed line tokenizes");
    assert_eq!(again, stored, "{:?}", source);
}

/// A directory for this test only
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bbc_issue_179_{}_{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir is created");
    dir
}

#[test]
fn test_rem_text_lists_as_typed() {
    let source = "10 REM a @ b, c;d  --  \"x\"";
    assert_eq!(listed(source), source);
    assert_same_tokens_after_list(source);
    let (result, output) = run(&[source, "20 PRINT \"A\""]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "A\n");
}

#[test]
fn test_rem_with_no_text_lists_as_typed() {
    assert_eq!(listed("10 REM"), "10 REM");
    assert_same_tokens_after_list("10 REM");
    let (result, output) = run(&["10 REM", "20 PRINT \"A\""]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "A\n");
}

#[test]
fn test_colon_after_rem_is_part_of_the_comment() {
    assert_eq!(listed("10 REM: x"), "10 REM: x");
    assert_same_tokens_after_list("10 REM: x");
    let (result, output) = run(&["10 REM: PRINT \"no\"", "20 PRINT \"A\""]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "A\n");
}

#[test]
fn test_rem_after_a_statement_runs_and_keeps_its_text() {
    // LIST shows a colon with no spaces, as in `X = 1:REM a` (issue 178)
    assert_eq!(listed("10 PRINT 1 : REM a:b"), "10 PRINT 1:REM a:b");
    assert_eq!(listed("10 PRINT 1:REM a:b"), "10 PRINT 1:REM a:b");
    assert_same_tokens_after_list("10 PRINT 1 : REM a:b");
    let (result, output) = run(&["10 PRINT 1 : REM a:b", "20 PRINT 2"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "         1\n         2\n");
}

#[test]
fn test_rem_text_that_is_not_ascii_lists_as_typed() {
    let source = "10 REM café → ☃ ½";
    assert_eq!(listed(source), source);
    assert_same_tokens_after_list(source);
    let (result, output) = run(&[source, "20 PRINT \"A\""]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "A\n");
}

#[test]
fn test_apostrophe_comment_still_lists_as_an_apostrophe() {
    assert_eq!(listed("10 ' note"), "10 ' note");
    assert_eq!(listed("10 REM ' note"), "10 REM ' note");
    assert_same_tokens_after_list("10 REM ' note");
}

#[test]
fn test_save_and_load_keep_the_rem_text() {
    let dir = temp_dir("save_load");
    let (_, output) = run_in_dir(
        &dir,
        &[
            "10 REM a @ b, c;d  --  \"x\"",
            "20 X = 1:REM a:b",
            "30 REM café ☃",
            "40 PRINT X",
            "SAVE \"rem.bas\"",
            "NEW",
            "LOAD \"rem.bas\"",
            "LIST",
            "RUN",
            "EXIT",
        ],
    );
    let saved = std::fs::read_to_string(dir.join("rem.bas")).expect("file is saved");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        saved.contains("10 REM a @ b, c;d  --  \"x\""),
        "{:?}",
        saved
    );
    assert!(saved.contains("20 X = 1:REM a:b"), "{:?}", saved);
    assert!(saved.contains("30 REM café ☃"), "{:?}", saved);
    assert!(
        output.contains("10 REM a @ b, c;d  --  \"x\"\n"),
        "{:?}",
        output
    );
    assert!(output.contains("20 X = 1:REM a:b\n"), "{:?}", output);
    assert!(output.contains("30 REM café ☃\n"), "{:?}", output);
    assert!(output.contains("1\n"), "{:?}", output);
    assert!(!output.contains("rror"), "{:?}", output);
}

#[test]
fn test_rem_in_an_if_branch_runs_as_before() {
    let (result, output) = run(&[
        "10 X = 1",
        "20 IF X = 1 THEN REM yes: ELSE PRINT \"no\"",
        "30 IF X = 2 THEN PRINT \"two\" ELSE REM not two",
        "40 IF X = 1 THEN PRINT \"one\" : REM a:b",
        "50 PRINT \"end\"",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "one\nend\n");
}

#[test]
fn test_rem_in_a_case_block_runs_as_before() {
    let (result, output) = run(&[
        "10 X = 2",
        "20 CASE X OF REM pick",
        "30 WHEN 1 : REM one",
        "40 PRINT \"one\"",
        "50 REM between: WHEN",
        "60 WHEN 2 : REM two",
        "70 PRINT \"two\"",
        "80 OTHERWISE",
        "90 PRINT \"other\"",
        "100 ENDCASE",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "two\n");
}

#[test]
fn test_rem_after_data_runs_as_before() {
    let (result, output) = run(&[
        "10 REM DATA 9, 9",
        "20 DATA 1, 2 : REM x",
        "30 READ A, B$",
        "40 PRINT A;\" \";B$",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "         1 2 : REM x\n");
}

#[test]
fn test_rem_in_a_for_each_block_runs_as_before() {
    let (result, output) = run(&[
        "10 L = NEWLIST",
        "20 LISTADD L, 1",
        "30 LISTADD L, 2",
        "40 FOR EACH V IN L : REM loop: NEXT",
        "50 REM NEXT",
        "60 PRINT V",
        "70 NEXT",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "         1\n         2\n");
}
