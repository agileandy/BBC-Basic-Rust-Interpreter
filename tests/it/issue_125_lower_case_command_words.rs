// Issue #125: a command word in lower case is a variable name when the line
// is not a correct command, such as `delete = 5`.

use crate::common::run_binary_read_first as run_binary;
use crate::common::run_in_dir;
use std::path::PathBuf;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bbc_issue_125_{}_{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir is made");
    dir
}

#[test]
fn test_variable_and_array_named_delete_still_work() {
    let output = run_binary(
        "delete = 5\nPRINT delete\nDIM delete(3)\ndelete(2) = 7\nPRINT delete(2)\nEXIT\n",
    );

    assert!(!output.contains("Error"), "output: {output}");
    assert!(output.contains("> 5\n"), "output: {output}");
    assert!(output.contains("> 7\n"), "output: {output}");
}

#[test]
fn test_variable_and_array_named_renumber_still_work() {
    let output = run_binary(
        "renumber = 6\nPRINT renumber\nDIM renumber(3)\nrenumber(2) = 8\nPRINT renumber(2)\nEXIT\n",
    );

    assert!(!output.contains("Error"), "output: {output}");
    assert!(output.contains("> 6\n"), "output: {output}");
    assert!(output.contains("> 8\n"), "output: {output}");
}

#[test]
fn test_variable_and_array_named_auto_still_work() {
    let output =
        run_binary("auto = 7\nPRINT auto\nDIM auto(3)\nauto(2) = 9\nPRINT auto(2)\nEXIT\n");

    assert!(!output.contains("Error"), "output: {output}");
    assert!(output.contains("> 7\n"), "output: {output}");
    assert!(output.contains("> 9\n"), "output: {output}");
}

#[test]
fn test_mixed_case_word_with_bad_argument_is_a_statement() {
    let output =
        run_binary("Delete = 1\nReNumber = 2\nAuto = 3\nPRINT Delete + ReNumber + Auto\nEXIT\n");

    assert!(!output.contains("Error"), "output: {output}");
    assert!(output.contains("> 6\n"), "output: {output}");
}

#[test]
fn test_lower_case_delete_with_a_range_still_deletes() {
    let output = run_binary("10 PRINT 1\n20 PRINT 2\n30 PRINT 3\ndelete 20, 20\nLIST\nEXIT\n");

    assert!(output.contains("10 PRINT 1\n"), "output: {output}");
    assert!(!output.contains("20 PRINT 2"), "output: {output}");
    assert!(output.contains("30 PRINT 3\n"), "output: {output}");
}

#[test]
fn test_lower_case_renumber_with_arguments_still_renumbers() {
    let output = run_binary("10 PRINT 1\n20 PRINT 2\nrenumber 100, 5\nLIST\nEXIT\n");

    assert!(output.contains("100 PRINT 1\n"), "output: {output}");
    assert!(output.contains("105 PRINT 2\n"), "output: {output}");
}

#[test]
fn test_lower_case_auto_with_arguments_still_numbers_lines() {
    let output = run_binary("auto 200, 5\nPRINT 1\nPRINT 2\n\nLIST\nEXIT\n");

    assert!(output.contains("200 PRINT 1\n"), "output: {output}");
    assert!(output.contains("205 PRINT 2\n"), "output: {output}");
}

#[test]
fn test_lower_case_commands_keep_their_range_errors() {
    let output = run_binary("10 PRINT 1\ndelete 40, 20\nauto 10, 0\nEXIT\n");

    assert!(
        output.contains("Error: Syntax error: DELETE start is more than end\n"),
        "output: {output}"
    );
    assert!(output.contains("Error: Silly\n"), "output: {output}");
}

#[test]
fn test_upper_case_commands_with_bad_argument_still_give_error() {
    let output = run_binary("DELETE = 5\nRENUMBER = 6\nAUTO = 7\nEXIT\n");

    assert!(
        output.contains("Error: Syntax error: use DELETE start, end\n"),
        "output: {output}"
    );
    assert_eq!(
        output.matches("Error: Syntax error\n").count(),
        2,
        "output: {output}"
    );
}

#[test]
fn test_variables_named_save_load_and_chain_still_work() {
    let dir = temp_dir("file_words");
    let (_, output) = run_in_dir(
        &dir,
        &[
            "10 PRINT 1",
            "save = 5",
            "load = 6",
            "chain = 7",
            "PRINT save + load + chain",
            "EXIT",
        ],
    );

    assert!(!output.contains("Error"), "output: {output}");
    assert!(output.contains("> 18\n"), "output: {output}");
    assert!(!dir.join("= 5.bbas").exists(), "SAVE wrote a file");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_lower_case_save_load_and_chain_still_work() {
    let dir = temp_dir("file_commands");
    let (_, output) = run_in_dir(
        &dir,
        &[
            "10 PRINT 42",
            "save \"prog\"",
            "NEW",
            "load \"prog\"",
            "LIST",
            "NEW",
            "chain \"prog\"",
            "EXIT",
        ],
    );

    assert!(output.contains("Saved to prog.bbas\n"), "output: {output}");
    assert!(output.contains("10 PRINT 42\n"), "output: {output}");
    assert_eq!(
        output.matches("Loaded from prog.bbas\n").count(),
        2,
        "output: {output}"
    );
    assert!(output.contains("42\n"), "output: {output}");
    let _ = std::fs::remove_dir_all(&dir);
}
