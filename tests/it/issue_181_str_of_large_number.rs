//! Issue #181: STR$ of a real value gives the text that PRINT shows for the
//! same value. A large whole number does not stop at 2147483647.

use crate::common::run_lines as run;

/// Run one PRINT line and give the output
fn printed(statement: &str) -> String {
    let (result, output) = run(&[statement]);
    assert_eq!(result, Ok(()), "{:?}", statement);
    output
}

#[test]
fn test_str_of_large_real_values_uses_print_format() {
    let (result, output) = run(&[
        "10 X = 1E10",
        "20 PRINT STR$(X)",
        "30 PRINT STR$(2147483648)",
        "40 Y = -1E10",
        "50 PRINT STR$(Y)",
        "60 PRINT STR$(1E10)",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "1E10\n2147483648\n-1E10\n1E10\n");
}

#[test]
fn test_str_gives_the_same_text_as_print() {
    // Each value has 9 significant digits or less. STR$ has 10 digits and
    // PRINT has 9 (issue 197), so a value such as 2147483648 is different.
    for value in ["1E10", "-1E10", "12", "1.5", "123456789", "-3"] {
        let (result, output) = run(&[
            &format!("10 X = {}", value),
            "20 PRINT STR$(X)",
            "30 PRINT X;",
        ]);
        assert_eq!(result, Ok(()), "{}", value);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 2, "{}: {:?}", value, output);
        // PRINT puts the number at the right of a field of 10 (issue #91)
        assert_eq!(format!("{:>10}", lines[0]), lines[1], "{}", value);
    }
}

#[test]
fn test_str_of_small_values_does_not_change() {
    assert_eq!(printed("10 PRINT STR$(12)"), "12\n");
    assert_eq!(printed("10 PRINT STR$(1.5)"), "1.5\n");
    assert_eq!(printed("10 X = 12 : PRINT STR$(X)"), "12\n");
    assert_eq!(printed("10 X = 6 / 4 : PRINT STR$(X)"), "1.5\n");
}

#[test]
fn test_str_of_integer_variable_does_not_change() {
    assert_eq!(
        printed("10 X% = 2147483647 : PRINT STR$(X%)"),
        "2147483647\n"
    );
    assert_eq!(printed("10 X% = -42 : PRINT STR$(X%)"), "-42\n");
}

#[test]
fn test_str_round_trips_through_val_and_len() {
    assert_eq!(
        printed("10 X = 123456 : PRINT VAL(STR$(X))"),
        "    123456\n"
    );
    assert_eq!(printed("10 X = 1E10 : PRINT VAL(STR$(X))"), "      1E10\n");
    assert_eq!(printed("10 X = 1E10 : PRINT LEN STR$(X)"), "         4\n");
}

#[test]
fn test_str_hex_of_value_out_of_range_is_too_big() {
    let (result, _) = run(&["10 X = 1E10", "20 PRINT STR$~(X)"]);
    assert_eq!(result, Err("Too big at line 20".to_string()));
    let (result, _) = run(&["10 X = 1E10", "20 PRINT ~X"]);
    assert_eq!(result, Err("Too big at line 20".to_string()));
}
