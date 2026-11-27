//! Issue 72: STRING$(n, A$) repeats the full text A$ n times, not only its
//! first character.

use crate::common::run_with_limit;

/// Print the expression between two bars and return the output.
fn print_of(expression: &str) -> String {
    let line = format!("10 PRINT \"|\";{expression};\"|\"");
    let (result, output) = run_with_limit(&[&line]);
    assert_eq!(result, Ok(()), "{expression} gives no error");
    output
}

#[test]
fn test_string_repeats_full_text() {
    assert_eq!(print_of("STRING$(3, \"AB\")"), "|ABABAB|\n");
}

#[test]
fn test_string_of_empty_text_is_empty() {
    assert_eq!(print_of("STRING$(2, \"\")"), "||\n");
}

#[test]
fn test_string_with_255_characters_works() {
    let expected = format!("|{}|\n", "A".repeat(255));
    assert_eq!(print_of("STRING$(255, \"A\")"), expected);
}

#[test]
fn test_string_with_zero_count_is_empty() {
    assert_eq!(print_of("STRING$(0, \"AB\")"), "||\n");
}

#[test]
fn test_string_longer_than_65535_characters_gives_string_too_long() {
    let (result, output) = run_with_limit(&[
        "10 ON ERROR GOTO 100",
        "20 PRINT STRING$(32768, \"AB\")",
        "30 END",
        "100 PRINT ERR",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "19");
}
