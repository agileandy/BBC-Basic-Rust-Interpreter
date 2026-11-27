//! Issue #63: PRINT and STR$ show a value in hex, and VAL reads hex text.
//!
//! In BBC BASIC, `~` in a PRINT list makes the numeric items that follow it
//! print in hex. A comma or a semicolon makes the format decimal again.

use crate::common::run_lines as run;
use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::{detokenize, tokenize};

/// Run a program and return the output. The program must not give an error.
fn output_of(source: &[&str]) -> String {
    let (result, output) = run(source);
    assert_eq!(result, Ok(()), "output so far: {:?}", output);
    output
}

#[test]
fn test_print_tilde_255_shows_ff() {
    assert_eq!(output_of(&["10 PRINT ~255"]), "FF\n");
}

#[test]
fn test_print_tilde_minus_1_shows_32_bit_form() {
    assert_eq!(output_of(&["10 PRINT ~-1"]), "FFFFFFFF\n");
}

#[test]
fn test_print_tilde_hex_literal_shows_same_digits() {
    assert_eq!(output_of(&["10 PRINT ~&1F"]), "1F\n");
}

#[test]
fn test_print_tilde_variable_and_real() {
    assert_eq!(
        output_of(&["10 A% = 4096", "20 B = 31.9", "30 PRINT ~A%;\" \";~B"]),
        "1000 1F\n"
    );
}

#[test]
fn test_print_str_tilde_gives_hex_text() {
    assert_eq!(output_of(&["10 PRINT STR$~(31)"]), "1F\n");
}

#[test]
fn test_str_tilde_result_is_a_string() {
    assert_eq!(
        output_of(&["10 A$ = STR$~(255) + \"!\"", "20 PRINT A$; LEN(A$)"]),
        "FF!3\n"
    );
}

#[test]
fn test_val_reads_hex_text() {
    assert_eq!(output_of(&["10 PRINT VAL(\"&1F\")"]), "31\n");
}

#[test]
fn test_input_reads_hex_reply() {
    let mut executor = Executor::new();
    let mut program = ProgramStore::new();
    for line in ["10 INPUT A%, B", "20 PRINT A%; \" \"; B"] {
        program.store_line(tokenize(line).expect("line tokenizes"));
    }
    executor.queue_input("&1F");
    executor.queue_input("&FF");

    let result = run_program(&mut executor, &mut program);

    assert_eq!(result, Ok(()));
    assert!(
        executor.get_output().ends_with("31 255\n"),
        "output: {:?}",
        executor.get_output()
    );
}

#[test]
fn test_comma_makes_format_decimal_again() {
    // "FF" in zone 1, then 255 in decimal at column 10
    assert_eq!(output_of(&["10 PRINT ~255,255"]), "FF        255\n");
}

#[test]
fn test_semicolon_makes_format_decimal_again() {
    assert_eq!(output_of(&["10 PRINT ~10;10"]), "A10\n");
}

#[test]
fn test_tilde_after_separator_applies_to_next_item() {
    assert_eq!(output_of(&["10 PRINT 10;~10"]), "10A\n");
}

#[test]
fn test_tilde_does_not_change_a_string_item() {
    assert_eq!(output_of(&["10 PRINT ~\"AB\""]), "AB\n");
}

#[test]
fn test_tilde_in_string_prints_as_typed() {
    assert_eq!(output_of(&["10 PRINT \"~\""]), "~\n");
}

#[test]
fn test_print_file_writes_hex_text() {
    let dir = std::env::temp_dir().join(format!("bbc_issue_63_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir is created");
    let file = dir.join("hex.txt").to_string_lossy().into_owned();
    let open_out = format!("10 F% = OPENOUT(\"{}\")", file);

    output_of(&[&open_out, "20 PRINT#F%, ~255;255", "30 CLOSE#F%"]);

    assert_eq!(std::fs::read_to_string(&file).unwrap(), "FF255\n");
}

#[test]
fn test_list_shows_tilde_as_typed() {
    for source in ["10 PRINT ~255", "10 PRINT STR$~(31)", "10 PRINT ~A%,~B"] {
        let listed = detokenize(&tokenize(source).unwrap()).unwrap();
        assert_eq!(listed, source);
    }
}
