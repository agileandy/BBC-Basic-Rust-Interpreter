//! Tests that LIST shows one line or a range of lines (issue #111).

use crate::common::run_binary_read_first as run_binary;
use bbc_basic_interpreter::program::{parse_list_range, ProgramStore};
use bbc_basic_interpreter::tokenizer::tokenize;

const PROGRAM: &str = "10 PRINT \"ONE\"\n20 PRINT \"TWO\"\n30 PRINT \"THREE\"\n40 PRINT \"FOUR\"\n50 PRINT \"FIVE\"\n";

fn store() -> ProgramStore {
    let mut store = ProgramStore::new();
    for line in ["10 END", "20 END", "30 END", "40 END", "50 END"] {
        store.store_line(tokenize(line).unwrap());
    }
    store
}

fn listed_numbers(store: &ProgramStore, start: u16, end: u16) -> Vec<u16> {
    store
        .list_range(start, end)
        .into_iter()
        .map(|(n, _)| n)
        .collect()
}

/// The text from the line `LIST` up to the next prompt
fn listing(output: &str) -> String {
    let after = output
        .split("> ")
        .nth(6)
        .expect("output has the LIST prompt");
    after.to_string()
}

#[test]
fn test_parse_no_argument_is_all_lines() {
    assert_eq!(parse_list_range(""), Ok((0, 65535)));
    assert_eq!(parse_list_range("  "), Ok((0, 65535)));
}

#[test]
fn test_parse_one_number_is_that_line() {
    assert_eq!(parse_list_range(" 60"), Ok((60, 60)));
    assert_eq!(parse_list_range("60"), Ok((60, 60)));
}

#[test]
fn test_parse_two_numbers_is_a_range() {
    assert_eq!(parse_list_range(" 10,60"), Ok((10, 60)));
    assert_eq!(parse_list_range(" 10 , 60 "), Ok((10, 60)));
}

#[test]
fn test_parse_number_and_comma_is_from_that_line() {
    assert_eq!(parse_list_range(" 30,"), Ok((30, 65535)));
}

#[test]
fn test_parse_comma_and_number_is_up_to_that_line() {
    assert_eq!(parse_list_range(" ,30"), Ok((0, 30)));
    assert_eq!(parse_list_range(",30"), Ok((0, 30)));
}

#[test]
fn test_parse_bad_argument_is_syntax_error() {
    for text in [" X", " 10,X", " 10,20,30", " 70000", " -5", " 1 0"] {
        assert_eq!(
            parse_list_range(text),
            Err("Syntax error".to_string()),
            "argument {text:?}"
        );
    }
}

#[test]
fn test_list_range_gives_lines_from_start_to_end() {
    assert_eq!(listed_numbers(&store(), 20, 40), vec![20, 30, 40]);
    assert_eq!(listed_numbers(&store(), 30, 30), vec![30]);
    assert_eq!(listed_numbers(&store(), 0, 65535), vec![10, 20, 30, 40, 50]);
}

#[test]
fn test_list_range_with_no_lines_is_empty() {
    assert!(listed_numbers(&store(), 60, 60).is_empty());
    assert!(listed_numbers(&store(), 21, 29).is_empty());
    assert!(listed_numbers(&store(), 40, 20).is_empty());
}

#[test]
fn test_binary_list_shows_all_lines() {
    let output = run_binary(&format!("{PROGRAM}LIST\nEXIT\n"));

    assert_eq!(
        listing(&output),
        "10 PRINT \"ONE\"\n20 PRINT \"TWO\"\n30 PRINT \"THREE\"\n40 PRINT \"FOUR\"\n50 PRINT \"FIVE\"\n"
    );
}

#[test]
fn test_binary_list_one_number_shows_that_line() {
    let output = run_binary(&format!("{PROGRAM}LIST 30\nEXIT\n"));

    assert_eq!(listing(&output), "30 PRINT \"THREE\"\n");
}

#[test]
fn test_binary_list_two_numbers_shows_the_range() {
    let output = run_binary(&format!("{PROGRAM}LIST 20,40\nEXIT\n"));

    assert_eq!(
        listing(&output),
        "20 PRINT \"TWO\"\n30 PRINT \"THREE\"\n40 PRINT \"FOUR\"\n"
    );
}

#[test]
fn test_binary_list_number_and_comma_shows_from_that_line() {
    let output = run_binary(&format!("{PROGRAM}LIST 40,\nEXIT\n"));

    assert_eq!(listing(&output), "40 PRINT \"FOUR\"\n50 PRINT \"FIVE\"\n");
}

#[test]
fn test_binary_list_comma_and_number_shows_up_to_that_line() {
    let output = run_binary(&format!("{PROGRAM}LIST ,20\nEXIT\n"));

    assert_eq!(listing(&output), "10 PRINT \"ONE\"\n20 PRINT \"TWO\"\n");
}

#[test]
fn test_binary_list_with_no_spaces_shows_the_range() {
    let output = run_binary(&format!("{PROGRAM}LIST20,30\nEXIT\n"));

    assert_eq!(listing(&output), "20 PRINT \"TWO\"\n30 PRINT \"THREE\"\n");
}

#[test]
fn test_binary_list_line_that_does_not_exist_shows_nothing() {
    let output = run_binary(&format!("{PROGRAM}LIST 60\nEXIT\n"));

    assert_eq!(listing(&output), "");
}

#[test]
fn test_binary_list_bad_argument_gives_syntax_error() {
    let output = run_binary(&format!("{PROGRAM}LIST X\nEXIT\n"));

    assert_eq!(listing(&output), "Error: Syntax error\n");
}

#[test]
fn test_binary_lower_case_list_command_shows_the_range() {
    let output = run_binary(&format!("{PROGRAM}list 20,30\nEXIT\n"));

    assert_eq!(listing(&output), "20 PRINT \"TWO\"\n30 PRINT \"THREE\"\n");
}

#[test]
fn test_variable_and_array_named_list_still_work() {
    let output =
        run_binary("list = 5\nPRINT list\nDIM list(3)\nlist(2) = 7\nPRINT list(2)\nEXIT\n");

    assert!(!output.contains("Error"), "output: {output}");
    assert!(output.contains(">          5\n"), "output: {output}");
    assert!(output.contains(">          7\n"), "output: {output}");
}
