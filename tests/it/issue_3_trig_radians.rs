//! Issue 3: the angle functions use radians, as on a BBC Micro.

use crate::common::run_lines as run;

/// Assert that the BASIC expression is within 1e-9 of the expected BASIC value.
fn assert_close(expression: &str, expected: &str) {
    let calc = format!("10 R = {}", expression);
    let check = format!(
        "20 IF ABS(R - ({})) < 0.000000001 THEN PRINT \"OK\" ELSE PRINT R",
        expected
    );
    let (result, output) = run(&[&calc, &check]);

    assert_eq!(result, Ok(()), "program for {} runs", expression);
    assert_eq!(output, "OK\n", "{} is not {}", expression, expected);
}

#[test]
fn test_sin_takes_radians() {
    assert_close("SIN(PI / 2)", "1");
}

#[test]
fn test_cos_takes_radians() {
    assert_close("COS(PI)", "-1");
}

#[test]
fn test_tan_takes_radians() {
    assert_close("TAN(PI / 4)", "1");
}

#[test]
fn test_atn_gives_radians() {
    assert_close("ATN(1) * 4", "PI");
}

#[test]
fn test_sin_of_rad_90_is_one() {
    assert_close("SIN(RAD(90))", "1");
}

#[test]
fn test_deg_of_atn_1_is_45() {
    assert_close("DEG(ATN(1))", "45");
}
