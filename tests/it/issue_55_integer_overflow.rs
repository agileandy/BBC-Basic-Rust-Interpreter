//! Issue 55: an integer calculation that overflows gives an error, not a panic.

use crate::common::run_lines as run;
use crate::common::run_with_limit;
use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::parser::{BinaryOperator, Expression};

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

    assert_eq!(result, Err("Too big at line 10".to_string()));
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
    // A power is a real value (issue #175). PRINT shows 9 significant digits,
    // as BBC BASIC does, and an integer variable keeps the exact value.
    let (result, output) = run(&[
        "10 PRINT 2 ^ 30",
        "20 PRINT 2 ^ 0",
        "30 A% = 2 ^ 30 : PRINT A%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1.07374182E9\n1\n1073741824\n");
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

#[test]
fn test_for_loop_to_largest_integer_ends_after_last_value() {
    let (result, output) = run_with_limit(&[
        "10 FOR I% = 2147483646 TO 2147483647",
        "20 PRINT I%",
        "30 NEXT I%",
        "40 PRINT \"DONE \"; I%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "2147483646\n2147483647\nDONE 2147483647\n");
}

#[test]
fn test_for_loop_down_to_smallest_integer_ends_after_last_value() {
    let (result, output) = run_with_limit(&[
        "10 FOR I% = -2147483647 TO -2147483647 - 1 STEP -1",
        "20 PRINT I%",
        "30 NEXT I%",
        "40 PRINT \"DONE \"; I%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "-2147483647\n-2147483648\nDONE -2147483648\n");
}

#[test]
fn test_for_loop_with_step_past_largest_integer_ends() {
    let (result, output) = run_with_limit(&[
        "10 FOR I% = 2147483640 TO 2147483647 STEP 5",
        "20 PRINT I%",
        "30 NEXT I%",
        "40 PRINT \"DONE \"; I%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "2147483640\n2147483645\nDONE 2147483645\n");
}
