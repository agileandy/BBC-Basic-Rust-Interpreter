//! Issue 52: a number that starts with & is a hex number.

use bbc_basic_interpreter::error::BBCBasicError;
use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::{detokenize, tokenize};

/// Store the source lines, run the program and return the result and the output.
fn run(source: &[&str]) -> (Result<(), String>, String) {
    let mut executor = Executor::new();
    let mut program = ProgramStore::new();
    for line in source {
        program.store_line(tokenize(line).expect("line tokenizes"));
    }
    let result = run_program(&mut executor, &mut program);
    (result, executor.get_output().to_string())
}

#[test]
fn test_print_hex_literal_shows_decimal_value() {
    let (result, output) = run(&["10 PRINT &1F"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "31");
}

#[test]
fn test_print_eight_f_hex_literal_shows_minus_one() {
    let (result, output) = run(&["10 PRINT &FFFFFFFF"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "-1");
}

#[test]
fn test_hex_literal_compares_equal_to_decimal() {
    let (result, output) = run(&["10 IF &10 = 16 THEN PRINT \"OK\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "OK\n");
}

#[test]
fn test_hex_literal_with_e_digit_is_not_an_exponent() {
    let (result, output) = run(&["10 A = &1E3 + 1", "20 PRINT A"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "484");
}

#[test]
fn test_ampersand_with_no_hex_digit_gives_syntax_error() {
    assert!(matches!(
        tokenize("10 PRINT &"),
        Err(BBCBasicError::SyntaxError { .. })
    ));
    assert!(matches!(
        tokenize("10 PRINT &G"),
        Err(BBCBasicError::SyntaxError { .. })
    ));
}

#[test]
fn test_hex_literal_with_nine_digits_gives_too_big() {
    assert_eq!(
        tokenize("10 PRINT &123456789").map(|_| ()),
        Err(BBCBasicError::TooBig)
    );
}

#[test]
fn test_list_shows_hex_literal_in_a_form_that_reads_back() {
    let listed = detokenize(&tokenize("10 PRINT &1F + &FFFFFFFF").unwrap()).unwrap();
    let reread = detokenize(&tokenize(&listed).unwrap()).unwrap();

    assert_eq!(listed, "10 PRINT 31 + -1");
    assert_eq!(reread, listed);
}
