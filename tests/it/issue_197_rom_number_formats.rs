//! Issue #197: 3 number formats of the BBC BASIC 2 ROM.
//!
//! N1: STR$ with byte 4 of `@%` at 0 uses the general format with 10 digits
//! (ROM &B0B1 to &B0B3, then &9EF9).
//! N2: in the general format, a value below 0.1 prints in exponent form
//! (ROM &9FC3 to &9FCD).
//! N3: in the general format, a value with an exponent that is not below the
//! count of digits prints in exponent form (ROM &9FB7). An integer goes
//! through the same routine, because the ROM changes it to a real value
//! first (ROM &9F09 to &9F0C).

use crate::common::{run_binary_read_first, run_lines as run};
use std::path::PathBuf;

/// Run the program lines and give the output. The program must not fail.
fn printed(source: &[&str]) -> String {
    let (result, output) = run(source);
    assert_eq!(result, Ok(()), "{:?}: {}", source, output);
    output
}

#[test]
fn test_str_gives_ten_digits() {
    assert_eq!(printed(&["10 PRINT STR$(PI)"]), "3.141592654\n");
    assert_eq!(printed(&["10 PRINT STR$(1/3)"]), "0.3333333333\n");
    assert_eq!(printed(&["10 PRINT STR$(1234567890)"]), "1234567890\n");
    assert_eq!(
        printed(&["10 A% = 1234567890 : PRINT STR$(A%)"]),
        "1234567890\n"
    );
    assert_eq!(printed(&["10 PRINT STR$(12345678901)"]), "1.23456789E10\n");
    assert_eq!(printed(&["10 PRINT STR$(-2147483648)"]), "-2147483648\n");
}

#[test]
fn test_str_gives_exponent_form_below_one_tenth() {
    assert_eq!(printed(&["10 PRINT STR$(0.05)"]), "5E-2\n");
    assert_eq!(printed(&["10 PRINT STR$(0.1)"]), "0.1\n");
}

#[test]
fn test_value_below_one_tenth_prints_in_exponent_form() {
    assert_eq!(printed(&["10 PRINT 0.05"]), "      5E-2\n");
    assert_eq!(printed(&["10 PRINT ;0.05"]), "5E-2\n");
    assert_eq!(printed(&["10 PRINT ;-0.05"]), "-5E-2\n");
    assert_eq!(printed(&["10 PRINT ;0.001"]), "1E-3\n");
    assert_eq!(printed(&["10 PRINT ;0.0123"]), "1.23E-2\n");
    assert_eq!(printed(&["10 PRINT ;1E-7"]), "1E-7\n");
}

#[test]
fn test_value_from_one_tenth_prints_in_plain_form() {
    assert_eq!(printed(&["10 PRINT ;0.1"]), "0.1\n");
    assert_eq!(printed(&["10 PRINT ;0.123"]), "0.123\n");
    assert_eq!(printed(&["10 PRINT ;-0.1"]), "-0.1\n");
    // The test is on the rounded value: this value rounds to 0.1
    assert_eq!(printed(&["10 PRINT ;0.09999999999"]), "0.1\n");
}

#[test]
fn test_whole_number_of_ten_digits_prints_in_exponent_form() {
    assert_eq!(printed(&["10 PRINT ;1234567890.0"]), "1.23456789E9\n");
    assert_eq!(printed(&["10 PRINT ;1234567890"]), "1.23456789E9\n");
    assert_eq!(printed(&["10 PRINT 1234567890"]), "1.23456789E9\n");
    assert_eq!(printed(&["10 PRINT ;2 ^ 30"]), "1.07374182E9\n");
    assert_eq!(printed(&["10 PRINT ;1000000000"]), "1E9\n");
}

#[test]
fn test_integer_variable_of_ten_digits_prints_in_exponent_form() {
    assert_eq!(
        printed(&["10 A% = 1234567890 : PRINT ;A%"]),
        "1.23456789E9\n"
    );
    assert_eq!(printed(&["10 PRINT ;&7FFFFFFF"]), "2.14748365E9\n");
    assert_eq!(
        printed(&["10 A% = &80000000 : PRINT ;A%"]),
        "-2.14748365E9\n"
    );
    assert_eq!(printed(&["10 A% = 1000000000 : PRINT ;A% DIV 1"]), "1E9\n");
}

#[test]
fn test_whole_number_of_nine_digits_prints_in_plain_form() {
    assert_eq!(printed(&["10 PRINT ;123456789"]), "123456789\n");
    assert_eq!(printed(&["10 A% = -999999999 : PRINT ;A%"]), "-999999999\n");
    assert_eq!(printed(&["10 PRINT ;123456789.0"]), "123456789\n");
}

#[test]
fn test_ten_digit_format_prints_all_ten_digits() {
    assert_eq!(
        printed(&["10 @% = &A0A : PRINT ;1234567890"]),
        "1234567890\n"
    );
    assert_eq!(
        printed(&["10 @% = &A0A : A% = 1234567890 : PRINT ;A%"]),
        "1234567890\n"
    );
}

#[test]
fn test_val_of_str_gives_the_same_value() {
    let values = [
        "0",
        "1",
        "-1",
        "0.1",
        "0.05",
        "-0.05",
        "0.001",
        "1E-7",
        "0.5",
        "2.5",
        "-3.75",
        "100",
        "123456789",
        "1234567890",
        "-1234567890",
        "9876543210",
        "1E20",
        "1.5E-20",
        "2 ^ 30",
        "0.0999999999",
    ];
    for value in values {
        let line = format!("10 X = {} : IF VAL(STR$(X)) = X PRINT \"same\"", value);
        assert_eq!(printed(&[&line]), "same\n", "VAL(STR$({}))", value);
    }
}

#[test]
fn test_print_file_text_does_not_change() {
    let dir: PathBuf =
        std::env::temp_dir().join(format!("bbc_issue_197_file_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir is created");
    let file = dir.join("data.txt").to_string_lossy().into_owned();
    let open_out = format!("10 H = OPENOUT(\"{}\")", file);
    printed(&[
        &open_out,
        "20 PRINT#H, 0.05, 1234567890, 1234567890.0, PI",
        "30 CLOSE#H",
    ]);

    let text = std::fs::read_to_string(&file).expect("file is readable");
    assert_eq!(text, "0.05\n1234567890\n1.23456789E9\n3.141592653589793\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_real_binary_prints_rom_formats() {
    let output =
        run_binary_read_first("10 PRINT ;0.05 : PRINT ;1234567890 : PRINT STR$(PI)\nRUN\nEXIT\n");
    assert!(output.contains("5E-2\n"), "{}", output);
    assert!(output.contains("1.23456789E9\n"), "{}", output);
    assert!(output.contains("3.141592654\n"), "{}", output);
}
