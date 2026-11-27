//! Issue 76: the memory operators `?` (one byte) and `!` (four bytes).
//!
//! The tests use the address &3000, which is between TOP and HIMEM.

use crate::common::run_lines as run;
use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_typed_line;
use bbc_basic_interpreter::tokenizer::tokenize;

#[test]
fn test_write_and_read_a_byte() {
    let (result, output) = run(&["10 A% = &3000", "20 ?A% = 42", "30 PRINT ?A%"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "42\n");
}

#[test]
fn test_byte_keeps_the_low_8_bits() {
    let (result, output) = run(&[
        "10 A% = &3000",
        "20 ?A% = 300",
        "30 PRINT ?A%",
        "40 ?A% = -1",
        "50 PRINT ?A%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "44\n255\n");
}

#[test]
fn test_word_is_stored_low_byte_first() {
    let (result, output) = run(&[
        "10 A% = &3000",
        "20 !A% = &12345678",
        "30 PRINT ?A%",
        "40 PRINT A%?1",
        "50 PRINT A%?2",
        "60 PRINT A%?3",
        "70 PRINT !A% = &12345678",
    ]);

    assert_eq!(result, Ok(()));
    // &78, &56, &34, &12, then TRUE
    assert_eq!(output, "120\n86\n52\n18\n-1\n");
}

#[test]
fn test_negative_word_reads_back_the_same() {
    let (result, output) = run(&["10 A% = &3000", "20 !A% = -5", "30 PRINT !A%"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "-5\n");
}

#[test]
fn test_binary_forms_write_and_read_at_base_plus_offset() {
    let (result, output) = run(&[
        "10 A% = &3000",
        "20 A%?5 = 7",
        "30 PRINT ?(A% + 5)",
        "40 A%!8 = 100000",
        "50 PRINT !(A% + 8)",
        "60 PRINT A%?5 + A%!8",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "7\n100000\n100007\n");
}

#[test]
fn test_unary_write_with_bracketed_address() {
    let (result, output) = run(&["10 A% = &3000", "20 ?(A% + 1) = 9", "30 PRINT A%?1"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "9\n");
}

#[test]
fn test_memory_write_after_colon() {
    let (result, output) = run(&["10 A% = &3000 : !A% = 3 : PRINT !A%"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "3\n");
}

#[test]
fn test_memory_operators_in_a_loop() {
    let (result, output) = run(&[
        "10 B% = &3000",
        "20 FOR I% = 0 TO 3",
        "30 B%?I% = I% * 2",
        "40 NEXT I%",
        "50 PRINT !B%",
    ]);

    assert_eq!(result, Ok(()));
    // bytes 0, 2, 4, 6: &06040200
    assert_eq!(output, "100925952\n");
}

/// Assert that the run stopped at the line with an address error, such as
/// "Invalid address: $8000 at line 10".
fn assert_address_error(result: Result<(), String>, line: u16) {
    let message = result.expect_err("an address error");
    assert!(
        message.starts_with("Invalid address: $") && message.ends_with(&format!(" at line {line}")),
        "{message}"
    );
}

#[test]
fn test_read_outside_memory_gives_error() {
    let (result, _) = run(&["10 PRINT ?&8000"]);

    assert_address_error(result, 10);
}

#[test]
fn test_negative_address_gives_error() {
    let (result, _) = run(&["10 !-4 = 1"]);

    assert_address_error(result, 10);
}

#[test]
fn test_word_write_across_the_end_of_memory_changes_nothing() {
    let (result, _) = run(&["10 A% = &7FFE", "20 ?A% = 1", "30 !A% = -1"]);

    assert_address_error(result, 30);

    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 A% = &7FFE",
        "30 ?A% = 1",
        "40 !A% = -1",
        "100 PRINT ?A%",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n");
}

#[test]
fn test_typed_line_continues_after_address_error() {
    let mut executor = Executor::new();
    let program = ProgramStore::new();
    let typed = |text: &str| tokenize(text).expect("line tokenizes");

    let bad = run_typed_line(&mut executor, &program, &typed("PRINT ?&8000"));
    let write = run_typed_line(&mut executor, &program, &typed("?&3000 = 7"));
    let read = run_typed_line(&mut executor, &program, &typed("PRINT ?&3000"));

    assert!(bad.is_err());
    assert!(write.is_ok(), "{write:?}");
    assert!(read.is_ok(), "{read:?}");
    assert_eq!(executor.get_output(), "7\n");
}
