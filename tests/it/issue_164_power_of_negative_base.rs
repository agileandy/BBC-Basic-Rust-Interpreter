//! Issue #164: a negative base to a power with a fraction gives "Log range",
//! and an arithmetic result that is not finite gives "Too big".

use crate::common::run_with_limit as run;

/// Run `10 PRINT <expression>` and give the result and the output.
fn print_of(expression: &str) -> (Result<(), String>, String) {
    let line = format!("10 PRINT {}", expression);
    run(&[line.as_str(), "20 END"])
}

/// Run `20 PRINT <expression>` under a handler that prints ERR and ERL.
fn error_with_handler(expression: &str) -> (Result<(), String>, String) {
    let line = format!("20 PRINT {}", expression);
    run(&[
        "10 ON ERROR PRINT ERR; \" \"; ERL : END",
        line.as_str(),
        "30 END",
    ])
}

#[test]
fn test_negative_base_with_fraction_exponent_gives_log_range() {
    for expression in ["(-8) ^ 0.5", "(-8) ^ (1/3)", "(-2.5) ^ 1.5"] {
        assert_eq!(
            print_of(expression),
            (Err("Log range at line 10".to_string()), String::new()),
            "{}",
            expression
        );
    }
}

#[test]
fn test_log_range_from_power_is_caught_by_on_error() {
    assert_eq!(
        error_with_handler("(-8) ^ 0.5"),
        (Ok(()), "22 20\n".to_string())
    );
}

#[test]
fn test_negative_base_with_whole_exponent_is_valid() {
    for (expression, expected) in [
        ("(-2) ^ 3", "-8\n"),
        ("(-2) ^ -2", "0.25\n"),
        ("(-2) ^ 2.0", "4\n"),
        ("(-2.5) ^ 2", "6.25\n"),
        ("2 ^ -1", "0.5\n"),
    ] {
        assert_eq!(
            print_of(expression),
            (Ok(()), expected.to_string()),
            "{}",
            expression
        );
    }
}

#[test]
fn test_positive_base_powers_do_not_change() {
    for (expression, expected) in [
        ("2 ^ 0.5", "1.41421356\n"),
        ("2 ^ 30", "1073741824\n"),
        ("0 ^ 0", "1\n"),
        ("0 ^ 0.5", "0\n"),
    ] {
        assert_eq!(
            print_of(expression),
            (Ok(()), expected.to_string()),
            "{}",
            expression
        );
    }
}

#[test]
fn test_zero_to_a_negative_whole_power_gives_division_by_zero() {
    for expression in ["0 ^ -1", "0.0 ^ -2", "Z% ^ -1"] {
        assert_eq!(
            print_of(expression),
            (
                Err("Division by zero at line 10".to_string()),
                String::new()
            ),
            "{}",
            expression
        );
    }
    // An integer exponent in a variable uses the integer power.
    let (result, _) = run(&["10 N% = -1", "20 PRINT 0 ^ N%", "30 END"]);
    assert_eq!(result, Err("Division by zero at line 20".to_string()));
}

#[test]
fn test_result_that_is_not_finite_gives_too_big() {
    for expression in [
        "1E308 * 10",
        "1E308 / 1E-10",
        "1E308 + 1E308",
        "-1E308 - 1E308",
        "10.0 ^ 400",
        "0 ^ -0.5",
    ] {
        assert_eq!(
            print_of(expression),
            (Err("Too big at line 10".to_string()), String::new()),
            "{}",
            expression
        );
    }
}

#[test]
fn test_too_big_from_real_arithmetic_is_caught_by_on_error() {
    assert_eq!(
        error_with_handler("1E308 * 10"),
        (Ok(()), "20 20\n".to_string())
    );
}

#[test]
fn test_too_big_stops_assignment_to_a_real_variable() {
    let (result, output) = run(&["10 A = 1E308", "20 B = A * 10", "30 PRINT B", "40 END"]);
    assert_eq!(result, Err("Too big at line 20".to_string()));
    assert_eq!(output, "");
}
