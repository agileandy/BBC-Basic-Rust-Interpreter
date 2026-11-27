//! Issue #91: PRINT shows a number at the right of a field of 10 characters,
//! as a BBC Micro does. A `;` turns the field off for the items after it, and
//! a `,` turns it on again. A string is never padded.
//!
//! The expected text comes from the PRINT routine of the BBC BASIC 2 ROM
//! (&8D83 to &8E1F). A `,` prints spaces up to the next multiple of 10
//! columns, and prints none if the column is a multiple of 10 already.

use crate::common::{run_binary_read_first, run_lines as run};

/// Run the program lines and give the output
fn printed(source: &[&str]) -> String {
    let (result, output) = run(source);
    assert_eq!(result, Ok(()), "{:?}", source);
    output
}

/// Run one PRINT line, then print POS with no padding on the next line.
/// Give the output.
fn printed_with_pos(statement: &str) -> String {
    printed(&[
        &format!("10 {};", statement),
        "20 P% = POS : PRINT ;\"/\";P%",
    ])
}

#[test]
fn test_a_number_prints_at_the_right_of_a_field_of_10() {
    assert_eq!(printed(&["10 PRINT 42"]), "        42\n");
}

#[test]
fn test_a_semicolon_before_a_number_gives_no_padding() {
    assert_eq!(printed(&["10 PRINT ;42"]), "42\n");
}

#[test]
fn test_a_comma_puts_each_number_in_the_next_field() {
    assert_eq!(printed(&["10 PRINT 1,2"]), "         1         2\n");
}

#[test]
fn test_a_semicolon_turns_the_field_off_for_the_next_number() {
    assert_eq!(printed(&["10 PRINT 1;2"]), "         12\n");
}

#[test]
fn test_a_string_is_not_padded_and_a_number_after_a_comma_is() {
    assert_eq!(printed(&["10 PRINT \"A\",3"]), "A                  3\n");
}

#[test]
fn test_a_number_after_a_string_and_a_semicolon_is_not_padded() {
    assert_eq!(printed(&["10 PRINT \"A\";3"]), "A3\n");
}

#[test]
fn test_a_comma_turns_the_field_on_again_after_a_semicolon() {
    assert_eq!(
        printed(&["10 PRINT 3;\"A\",4"]),
        "         3A                  4\n"
    );
}

#[test]
fn test_a_negative_real_is_padded_with_its_minus_sign() {
    assert_eq!(printed(&["10 PRINT -1.5"]), "      -1.5\n");
}

#[test]
fn test_a_number_longer_than_the_field_prints_in_full() {
    assert_eq!(printed(&["10 PRINT 1234567890123"]), "1.23456789E12\n");
}

#[test]
fn test_a_hex_number_is_padded() {
    assert_eq!(printed(&["10 PRINT ~255"]), "        FF\n");
}

#[test]
fn test_a_hex_number_after_a_semicolon_is_not_padded() {
    assert_eq!(printed(&["10 PRINT ;~255"]), "FF\n");
}

#[test]
fn test_a_comma_on_a_field_boundary_prints_no_spaces() {
    assert_eq!(printed(&["10 PRINT 1,,2"]), "         1         2\n");
    assert_eq!(printed(&["10 PRINT ,1"]), "         1\n");
    assert_eq!(printed(&["10 PRINT \"ABCDEFGHIJ\",\"X\""]), "ABCDEFGHIJX\n");
}

#[test]
fn test_numbers_with_no_separator_each_get_a_field() {
    assert_eq!(printed(&["10 PRINT 1 2"]), "         1         2\n");
    assert_eq!(printed(&["10 PRINT ;1 2"]), "12\n");
}

#[test]
fn test_tab_spc_and_apostrophe_do_not_change_the_field() {
    assert_eq!(printed(&["10 PRINT ;1 TAB(5) 2"]), "1    2\n");
    assert_eq!(printed(&["10 PRINT ;1 SPC(2) 2"]), "1  2\n");
    assert_eq!(printed(&["10 PRINT ;1'2"]), "1\n2\n");
    assert_eq!(printed(&["10 PRINT 1'2"]), "         1\n         2\n");
    assert_eq!(printed(&["10 PRINT TAB(3) 1"]), "            1\n");
}

#[test]
fn test_pos_counts_the_padding() {
    for (statement, expected) in [
        ("PRINT 42", "        42/10\n"),
        ("PRINT ;42", "42/2\n"),
        ("PRINT 1,2", "         1         2/20\n"),
        ("PRINT 1;2", "         12/11\n"),
        ("PRINT \"A\",3", "A                  3/20\n"),
        ("PRINT \"A\";3", "A3/2\n"),
        ("PRINT 3;\"A\",4", "         3A                  4/30\n"),
        ("PRINT -1.5", "      -1.5/10\n"),
        ("PRINT 1234567890123", "1.23456789E12/13\n"),
        ("PRINT ~255", "        FF/10\n"),
        ("PRINT ;~255", "FF/2\n"),
        ("PRINT 1,,2", "         1         2/20\n"),
        ("PRINT 1 2", "         1         2/20\n"),
    ] {
        assert_eq!(printed_with_pos(statement), expected, "{}", statement);
    }
}

#[test]
fn test_a_number_from_a_function_or_eval_is_padded_and_a_string_is_not() {
    assert_eq!(
        printed(&[
            "10 PRINT FNn, FNs, EVAL(\"7\"), EVAL(\"\"\"B\"\"\")",
            "20 END",
            "30 DEF FNn = 5",
            "40 DEF FNs = \"S\"",
        ]),
        "         5S                  7B\n"
    );
}

#[test]
fn test_str_gives_no_padding() {
    assert_eq!(printed(&["10 PRINT STR$(42)"]), "42\n");
}

#[test]
fn test_a_typed_print_line_uses_the_field() {
    let output = run_binary_read_first("PRINT 42\nPRINT ;42\nEXIT\n");
    assert!(output.contains(">         42\n> 42\n"), "{:?}", output);
}
