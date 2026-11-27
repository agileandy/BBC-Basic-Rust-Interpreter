//! Issue 8: INPUT reads a numeric reply as BBC BASIC does, and VAL uses the same rule.

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;

/// Store the source lines, give the replies to INPUT, run the program and return
/// the result and the output.
fn run_with_replies(source: &[&str], replies: &[&str]) -> (Result<(), String>, String) {
    let mut executor = Executor::new();
    for reply in replies {
        executor.queue_input(reply);
    }
    let mut program = ProgramStore::new();
    for line in source {
        program.store_line(tokenize(line).expect("line tokenizes"));
    }
    let result = run_program(&mut executor, &mut program);
    (result, executor.get_output().to_string())
}

/// Give one reply to `INPUT <var>` after the variable holds 99, then print it.
fn input_one(var: &str, reply: &str) -> String {
    let init = format!("10 {} = 99", var);
    let input = format!("20 INPUT {}", var);
    let print = format!("30 PRINT {}", var);
    let (result, output) = run_with_replies(&[&init, &input, &print], &[reply]);
    assert_eq!(result, Ok(()));
    output
}

#[test]
fn test_real_input_letters_give_zero() {
    assert_eq!(input_one("A", "abc"), "0\n");
}

#[test]
fn test_real_input_leading_number_is_used() {
    assert_eq!(input_one("A", "12abc"), "12\n");
}

#[test]
fn test_real_input_empty_reply_gives_zero() {
    assert_eq!(input_one("A", ""), "0\n");
}

#[test]
fn test_real_input_decimal_reply() {
    assert_eq!(input_one("A", "3.5"), "3.5\n");
}

#[test]
fn test_real_input_negative_reply() {
    assert_eq!(input_one("A", "-2"), "-2\n");
}

#[test]
fn test_integer_input_letters_give_zero() {
    assert_eq!(input_one("A%", "abc"), "0\n");
}

#[test]
fn test_integer_input_leading_number_is_used() {
    assert_eq!(input_one("A%", "12abc"), "12\n");
}

#[test]
fn test_integer_input_empty_reply_gives_zero() {
    assert_eq!(input_one("A%", ""), "0\n");
}

#[test]
fn test_integer_input_decimal_reply_is_truncated() {
    assert_eq!(input_one("A%", "3.5"), "3\n");
}

#[test]
fn test_integer_input_negative_reply() {
    assert_eq!(input_one("A%", "-2"), "-2\n");
}

#[test]
fn test_string_input_keeps_text() {
    let (result, output) = run_with_replies(&["10 INPUT N$", "20 PRINT N$"], &["hello world"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "hello world\n");
}

#[test]
fn test_input_takes_replies_in_order() {
    let (result, output) = run_with_replies(
        &["10 INPUT A", "20 INPUT B%", "30 PRINT A", "40 PRINT B%"],
        &["4", "5"],
    );
    assert_eq!(result, Ok(()));
    assert_eq!(output, "4\n5\n");
}

/// Run `PRINT VAL("<text>")` and return the output.
fn print_val(text: &str) -> String {
    let line = format!("10 PRINT VAL(\"{}\")", text);
    let (result, output) = run_with_replies(&[&line], &[]);
    assert_eq!(result, Ok(()));
    output
}

#[test]
fn test_val_uses_leading_number() {
    assert_eq!(print_val("12abc"), "12\n");
}

#[test]
fn test_val_letters_give_zero() {
    assert_eq!(print_val("abc"), "0\n");
}

#[test]
fn test_val_ignores_leading_spaces() {
    assert_eq!(print_val(" 3.5"), "3.5\n");
}

#[test]
fn test_val_reads_exponent() {
    assert_eq!(print_val("1E3"), "1000\n");
}
