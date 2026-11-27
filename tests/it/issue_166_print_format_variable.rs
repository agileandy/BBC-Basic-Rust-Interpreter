//! Issue #166: the print format variable `@%`.
//!
//! The rules come from the number output routine of the BBC BASIC 2 ROM
//! (&9EDF to &A03F), the PRINT routine (&8D83 to &8E1F) and STR$ (&B0AC).
//! Byte 1 of `@%` is the field width, byte 2 the count of digits, byte 3 the
//! format (0 general, 1 exponent, 2 fixed) and byte 4 the STR$ flag.

use bbc_basic_interpreter::tokenizer::tokenize;

use crate::common::{run_binary_read_first, run_lines as run};

/// Run the program lines and give the output. The program must not fail.
fn printed(source: &[&str]) -> String {
    let (result, output) = run(source);
    assert_eq!(result, Ok(()), "{:?}: {}", source, output);
    output
}

#[test]
fn test_default_value_is_90a() {
    assert_eq!(printed(&["10 PRINT @%"]), "      2314\n");
    assert_eq!(printed(&["10 PRINT ~@%"]), "       90A\n");
}

#[test]
fn test_fixed_format_gives_decimal_places_in_field() {
    assert_eq!(printed(&["10 @% = &20209 : PRINT 2.5"]), "     2.50\n");
    assert_eq!(printed(&["10 @% = &20209 : PRINT -12.5"]), "   -12.50\n");
    assert_eq!(printed(&["10 @% = &20209 : PRINT 0"]), "     0.00\n");
}

#[test]
fn test_fixed_format_rounds_half_up() {
    assert_eq!(printed(&["10 @% = &20109 : PRINT 0.25"]), "      0.3\n");
    assert_eq!(printed(&["10 @% = &20109 : PRINT 9.96"]), "     10.0\n");
    assert_eq!(printed(&["10 @% = &20109 : PRINT 0.06"]), "      0.1\n");
}

#[test]
fn test_fixed_format_keeps_sign_of_negative_value_that_rounds_to_zero() {
    assert_eq!(printed(&["10 @% = &20209 : PRINT -0.001"]), "    -0.00\n");
}

#[test]
fn test_fixed_format_with_no_decimal_places_ends_with_point() {
    assert_eq!(printed(&["10 @% = &2000A : PRINT 2.5"]), "        3.\n");
}

#[test]
fn test_fixed_format_with_more_than_10_digits_uses_general_10() {
    assert_eq!(
        printed(&["10 @% = &20500 : PRINT 123456789.5"]),
        "123456789.5\n"
    );
}

#[test]
fn test_integer_follows_fixed_format() {
    assert_eq!(printed(&["10 @% = &20209 : PRINT 100"]), "   100.00\n");
    assert_eq!(
        printed(&["10 @% = &20209 : X% = -7 : PRINT X%"]),
        "    -7.00\n"
    );
}

#[test]
fn test_exponent_format_pads_exponent_to_4_characters() {
    assert_eq!(printed(&["10 @% = &1020A : PRINT 1234.5"]), "   1.2E3  \n");
    assert_eq!(printed(&["10 @% = &1030A : PRINT 0.00125"]), "  1.25E-3 \n");
    assert_eq!(printed(&["10 @% = &1030A : PRINT 1.5E12"]), "  1.50E12 \n");
    assert_eq!(printed(&["10 @% = &1020A : PRINT -1234.5"]), "  -1.2E3  \n");
}

#[test]
fn test_integer_and_zero_follow_exponent_format() {
    assert_eq!(printed(&["10 @% = &1030A : PRINT 5"]), "  5.00E0  \n");
    assert_eq!(printed(&["10 @% = &1030A : PRINT 0"]), "  0.00E0  \n");
}

#[test]
fn test_general_format_uses_count_of_digits() {
    assert_eq!(printed(&["10 @% = &40A : PRINT 3.14159"]), "     3.142\n");
    assert_eq!(printed(&["10 @% = &40A : PRINT -3.14159"]), "    -3.142\n");
    assert_eq!(printed(&["10 @% = &40A : PRINT 123456.7"]), "   1.235E5\n");
    assert_eq!(printed(&["10 @% = &40A : PRINT 2.5"]), "       2.5\n");
}

#[test]
fn test_count_of_digits_0_or_above_10_uses_10() {
    assert_eq!(printed(&["10 @% = &000A : PRINT PI"]), "3.141592654\n");
    assert_eq!(printed(&["10 @% = &0F0A : PRINT PI"]), "3.141592654\n");
}

#[test]
fn test_format_byte_above_2_uses_general_format() {
    assert_eq!(printed(&["10 @% = &30209 : PRINT 2.5"]), "      2.5\n");
}

#[test]
fn test_field_width_of_5_also_sets_comma_zones() {
    assert_eq!(printed(&["10 @% = &905 : PRINT 1, 2"]), "    1    2\n");
    assert_eq!(printed(&["10 @% = &905 : PRINT \"AB\", 2"]), "AB       2\n");
}

#[test]
fn test_field_width_of_0_gives_no_padding() {
    assert_eq!(printed(&["10 @% = &900 : PRINT 1, 2; 3"]), "123\n");
    assert_eq!(printed(&["10 @% = &900 : PRINT \"A\", \"B\""]), "AB\n");
}

#[test]
fn test_hex_output_uses_field_width_only() {
    assert_eq!(printed(&["10 @% = &20209 : PRINT ~@%"]), "    20209\n");
    assert_eq!(printed(&["10 @% = &20205 : PRINT ~255"]), "   FF\n");
}

#[test]
fn test_str_with_no_byte_4_uses_general_format() {
    assert_eq!(printed(&["10 @% = &20209 : PRINT STR$(2.5)"]), "2.5\n");
    assert_eq!(printed(&["10 @% = &20209 : PRINT STR$(100)"]), "100\n");
}

#[test]
fn test_str_with_byte_4_uses_format_with_no_field() {
    assert_eq!(printed(&["10 @% = &1020209 : PRINT STR$(2.5)"]), "2.50\n");
    assert_eq!(printed(&["10 @% = &1020209 : PRINT STR$(100)"]), "100.00\n");
    assert_eq!(
        printed(&["10 @% = &1020209 : X% = 7 : A$ = STR$(X%) : PRINT A$"]),
        "7.00\n"
    );
    assert_eq!(
        printed(&["10 @% = &1010209 : PRINT STR$(1234.5)"]),
        "1.2E3  \n"
    );
}

#[test]
fn test_at_percent_in_expression() {
    assert_eq!(
        printed(&["10 @% = &20209 : X% = @% AND &FF : @% = &90A : PRINT X%"]),
        "         9\n"
    );
    assert_eq!(
        printed(&["10 @% = @% + 1 : PRINT 2", "20 @% = &90A : PRINT 2"]),
        "          2\n         2\n"
    );
}

#[test]
fn test_lvar_lists_at_percent_only_when_changed() {
    let output = run_binary_read_first("LVAR\nPRINT \"MARK\"\n@% = &20209\nLVAR\nEXIT\n");
    let (before, after) = output.split_once("MARK").expect("output has the mark");
    assert!(!before.contains("@%"), "output: {output}");
    assert!(after.contains("@% = 131593\n"), "output: {output}");
}

#[test]
fn test_run_and_clear_keep_at_percent() {
    let output = run_binary_read_first("@% = &20209\n10 CLEAR\n20 PRINT 2.5\nRUN\nEXIT\n");
    assert!(output.contains("     2.50\n"), "output: {output}");
}

#[test]
fn test_print_at_percent_at_the_prompt() {
    let output = run_binary_read_first("PRINT @%\nEXIT\n");
    assert!(output.contains("      2314\n"), "output: {output}");
}

#[test]
fn test_other_use_of_at_sign_is_still_an_error() {
    match tokenize("10 PRINT @5") {
        Err(error) => assert_eq!(
            error.to_string(),
            "Syntax error at line 10: Unknown character '@'"
        ),
        Ok(line) => panic!("@5 tokenized as {:?}", line),
    }
}

#[test]
fn test_variable_and_array_named_at_still_work() {
    assert_eq!(
        printed(&["10 AT% = 4 : DIM AT%(2) : AT%(1) = 5 : PRINT AT%; AT%(1)"]),
        "         45\n"
    );
}
