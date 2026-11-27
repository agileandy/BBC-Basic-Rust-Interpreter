//! Tests for issue 2: division keeps its fraction part.

use crate::common::run_lines as run;

#[test]
fn test_print_division_keeps_fraction() {
    assert_eq!(
        run(&["10 PRINT 7 / 2"]),
        (Ok(()), "       3.5\n".to_string())
    );
}

#[test]
fn test_integer_variable_keeps_fraction_until_assignment() {
    assert_eq!(
        run(&["10 C% = 7 / 2 * 2", "20 PRINT C%"]),
        (Ok(()), "         7\n".to_string())
    );
}

#[test]
fn test_print_division_by_zero_reports_division_by_zero() {
    let (result, _) = run(&["10 PRINT 1 / 0"]);
    assert_eq!(result, Err("Division by zero at line 10".to_string()));
}

#[test]
fn test_print_evaluates_a_user_function_one_time() {
    let source = [
        "10 N% = 0",
        "20 PRINT FNhalf",
        "30 PRINT N%",
        "40 END",
        "100 DEF FNhalf",
        "110 N% = N% + 1",
        "120 = N% / 2",
    ];
    assert_eq!(
        run(&source),
        (Ok(()), "       0.5\n         1\n".to_string())
    );
}

#[test]
fn test_print_exact_division_shows_whole_number() {
    assert_eq!(
        run(&["10 PRINT 6 / 2"]),
        (Ok(()), "         3\n".to_string())
    );
}

#[test]
fn test_print_div_stays_integer() {
    assert_eq!(
        run(&["10 PRINT 7 DIV 2"]),
        (Ok(()), "         3\n".to_string())
    );
}

#[test]
fn test_print_integer_addition() {
    assert_eq!(
        run(&["10 PRINT 2 + 3"]),
        (Ok(()), "         5\n".to_string())
    );
}

#[test]
fn test_print_string_concatenation() {
    assert_eq!(
        run(&["10 PRINT \"A\" + \"B\""]),
        (Ok(()), "AB\n".to_string())
    );
}
