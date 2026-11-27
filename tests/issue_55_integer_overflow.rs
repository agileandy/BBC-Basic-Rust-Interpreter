//! Issue 55: an integer calculation that overflows gives an error, not a panic.

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::parser::{BinaryOperator, Expression};
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;

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

/// Run one statement after `ON ERROR` and return the output: the value of ERR,
/// or "NO ERROR" when the statement gives no error.
fn err_of(statement: &str) -> String {
    let line = format!("20 {statement}");
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        &line,
        "30 PRINT \"NO ERROR\"",
        "40 END",
        "100 PRINT ERR",
    ]);
    assert_eq!(result, Ok(()));
    output
}

#[test]
fn test_add_overflow_gives_too_big() {
    let (result, _) = run(&["10 A% = 2147483647 + 1"]);

    assert_eq!(result, Err("Runtime error at line 10: TooBig".to_string()));
}

#[test]
fn test_add_overflow_is_caught_by_on_error() {
    assert_eq!(err_of("A% = 2147483647 + 1"), "20\n");
}

#[test]
fn test_subtract_overflow_is_caught_by_on_error() {
    assert_eq!(err_of("A% = -2147483647 - 1 - 1"), "20\n");
}

#[test]
fn test_subtract_from_smallest_literal_is_caught_by_on_error() {
    assert_eq!(err_of("A% = -2147483648 - 1"), "20\n");
}

#[test]
fn test_multiply_overflow_is_caught_by_on_error() {
    assert_eq!(err_of("A% = 65536 * 65536"), "20\n");
}

#[test]
fn test_multiply_variable_overflow_is_caught_by_on_error() {
    assert_eq!(err_of("A% = 2147483647 : B% = A% * 2"), "20\n");
}

#[test]
fn test_power_overflow_is_caught_by_on_error() {
    assert_eq!(err_of("A% = 2 ^ 40"), "20\n");
}

#[test]
fn test_power_with_negative_exponent_gives_truncated_value() {
    let (result, output) = run(&["10 A% = 2 ^ -1", "20 B% = 4 ^ -1 * 8", "30 PRINT A%"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0\n");
}

#[test]
fn test_power_in_range_is_exact() {
    let (result, output) = run(&["10 PRINT 2 ^ 30", "20 PRINT 2 ^ 0"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1073741824\n1\n");
}

#[test]
fn test_mod_by_zero_is_caught_by_on_error() {
    assert_eq!(err_of("A% = 5 MOD 0"), "18\n");
}

#[test]
fn test_div_by_zero_is_caught_by_on_error() {
    assert_eq!(err_of("A% = 5 DIV 0"), "18\n");
}

#[test]
fn test_smallest_div_minus_one_is_caught_by_on_error() {
    assert_eq!(err_of("A% = -2147483647 - 1 : B% = A% DIV -1"), "20\n");
}

#[test]
fn test_smallest_mod_minus_one_gives_zero() {
    let (result, output) = run(&["10 A% = -2147483647 - 1", "20 PRINT A% MOD -1"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0\n");
}

#[test]
fn test_minus_smallest_integer_is_caught_by_on_error() {
    assert_eq!(err_of("A% = -2147483647 - 1 : B% = -A%"), "20\n");
}

/// Evaluate `left op right` as an integer. The parser gives no shift operator,
/// so the shift tests build the expression directly.
fn eval_shift(op: BinaryOperator, left: i32, right: i32) -> i32 {
    let expr = Expression::BinaryOp {
        op,
        left: Box::new(Expression::Integer(left)),
        right: Box::new(Expression::Integer(right)),
    };
    Executor::new()
        .eval_integer(&expr)
        .expect("shift gives a value")
}

#[test]
fn test_left_shift_by_40_gives_zero() {
    assert_eq!(eval_shift(BinaryOperator::LeftShift, 1, 40), 0);
    assert_eq!(eval_shift(BinaryOperator::LeftShift, 1, 32), 0);
}

#[test]
fn test_right_shift_by_40_gives_sign_fill() {
    assert_eq!(eval_shift(BinaryOperator::RightShift, 5, 40), 0);
    assert_eq!(eval_shift(BinaryOperator::RightShift, -5, 40), -1);
}

#[test]
fn test_largest_and_smallest_integers_work_with_no_overflow() {
    let (result, output) = run(&[
        "10 A% = 2147483646 + 1",
        "20 B% = -2147483647 - 1",
        "30 C% = 1073741823 * 2 + 1",
        "40 D% = A% + B%",
        "50 E% = -1073741824 * 2",
        "60 PRINT A%",
        "70 PRINT B%",
        "80 PRINT C%",
        "90 PRINT D%",
        "100 PRINT E%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(
        output,
        "2147483647\n-2147483648\n2147483647\n-1\n-2147483648\n"
    );
}

#[test]
fn test_abs_of_smallest_integer_is_caught_by_on_error() {
    assert_eq!(err_of("A% = -2147483647 - 1 : B% = ABS(A%)"), "20\n");
}
