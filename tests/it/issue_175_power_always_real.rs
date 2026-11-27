//! Issue #175: the power operator always gives a real result, as in BBC BASIC.

use crate::common::run_with_limit as run;

/// Run `10 PRINT <expression>` and give the result and the output.
fn print_of(expression: &str) -> (Result<(), String>, String) {
    let line = format!("10 PRINT {}", expression);
    run(&[line.as_str(), "20 END"])
}

#[test]
fn test_negative_exponent_in_a_variable_gives_a_fraction() {
    let (result, output) = run(&["10 N% = -1", "20 PRINT 2 ^ N%", "30 END"]);
    assert_eq!((result, output), (Ok(()), "0.5\n".to_string()));
}

#[test]
fn test_power_out_of_integer_range_prints_a_real_value() {
    for (expression, expected) in [("2 ^ 31", "2.14748365E9\n"), ("2 ^ 40", "1.09951163E12\n")] {
        assert_eq!(
            print_of(expression),
            (Ok(()), expected.to_string()),
            "{}",
            expression
        );
    }
}

#[test]
fn test_whole_number_powers_print_the_exact_value() {
    for (expression, expected) in [
        ("2 ^ 3", "8\n"),
        ("10 ^ 2", "100\n"),
        ("3 ^ 4", "81\n"),
        ("7 ^ 2", "49\n"),
        ("2 ^ 10", "1024\n"),
        ("10 ^ 8", "100000000\n"),
        ("3 ^ 2 + 1", "10\n"),
        ("(-2) ^ 3", "-8\n"),
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
fn test_whole_number_powers_convert_exactly_to_an_integer() {
    let (result, output) = run(&[
        "10 A% = 2 ^ 3",
        "20 B% = 10 ^ 2",
        "30 C% = 3 ^ 4",
        "40 D% = 10 ^ 9",
        "50 PRINT A%; \" \"; B%; \" \"; C%; \" \"; D%",
        "60 END",
    ]);
    assert_eq!(
        (result, output),
        (Ok(()), "8 100 81 1000000000\n".to_string())
    );
}

#[test]
fn test_power_used_as_an_integer_converts_the_real_value() {
    let (result, output) = run(&["10 PRINT 10 ^ 2 DIV 3; \" \"; 2 ^ 3 MOD 5", "20 END"]);
    assert_eq!((result, output), (Ok(()), "33 3\n".to_string()));
}

#[test]
fn test_assign_out_of_range_power_to_an_integer_gives_too_big() {
    let (result, output) = run(&["10 A% = 2 ^ 31", "20 END"]);
    assert_eq!(
        (result, output),
        (Err("Too big at line 10".to_string()), String::new())
    );
}
