//! Issue #152: a negative root, a log out of range and an exponent out of
//! range give the BBC BASIC error text and number.

use crate::common::run_with_limit as run;

/// Run `10 PRINT <expression>` with no handler and give the result.
fn error_with_no_handler(expression: &str) -> Result<(), String> {
    let line = format!("10 PRINT {}", expression);
    run(&[line.as_str(), "20 END"]).0
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
fn test_negative_root_with_no_handler() {
    for expression in ["SQR(-1)", "ASN(2)", "ASN(-1.5)", "ACS(2)", "ACS(-1.5)"] {
        assert_eq!(
            error_with_no_handler(expression),
            Err("-ve root at line 10".to_string()),
            "{}",
            expression
        );
    }
}

#[test]
fn test_negative_root_with_handler() {
    for expression in ["SQR(-1)", "ASN(2)", "ACS(2)"] {
        assert_eq!(
            error_with_handler(expression),
            (Ok(()), "        21 20\n".to_string()),
            "{}",
            expression
        );
    }
}

#[test]
fn test_log_range_with_no_handler() {
    for expression in ["LOG(-1)", "LOG(0)", "LN(0)", "LN(-1)"] {
        assert_eq!(
            error_with_no_handler(expression),
            Err("Log range at line 10".to_string()),
            "{}",
            expression
        );
    }
}

#[test]
fn test_log_range_with_handler() {
    for expression in ["LOG(-1)", "LOG(0)", "LN(0)", "LN(-1)"] {
        assert_eq!(
            error_with_handler(expression),
            (Ok(()), "        22 20\n".to_string()),
            "{}",
            expression
        );
    }
}

#[test]
fn test_exp_range_with_no_handler() {
    assert_eq!(
        error_with_no_handler("EXP(1000)"),
        Err("Exp range at line 10".to_string())
    );
}

#[test]
fn test_exp_range_with_handler() {
    assert_eq!(
        error_with_handler("EXP(1000)"),
        (Ok(()), "        24 20\n".to_string())
    );
}

#[test]
fn test_correct_values_are_unchanged() {
    let (result, output) = run(&[
        "10 PRINT SQR(0)",
        "20 PRINT LN(1)",
        "30 PRINT LOG(100)",
        "40 PRINT EXP(1)",
        "50 PRINT EXP(709) > 8E307",
        "60 PRINT ASN(1) = PI/2, ACS(-1) = PI",
        "70 END",
    ]);
    assert_eq!(
        (result, output),
        (
            Ok(()),
            "         0\n         0\n         2\n2.71828183\n        -1\n        -1        -1\n"
                .to_string()
        )
    );
}
