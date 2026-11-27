//! Issue #143: a built-in function of one value works with no brackets.
//! The function takes the next factor, so `SIN X + 1` is `(SIN X) + 1`.

use crate::common::{run_binary_read_first, run_with_limit as run};

/// Each built-in function of one value: the name, a literal argument, and
/// the text that PRINT gives for the result.
const FUNCTIONS: &[(&str, &str, &str)] = &[
    ("ABS", "-3", "3"),
    ("ACS", "1", "0"),
    ("ADVAL", "0", "0"),
    ("ASC", "\"A\"", "65"),
    ("ASN", "0", "0"),
    ("ATN", "0", "0"),
    ("CHR$", "65", "A"),
    ("COS", "0", "1"),
    ("DEG", "0", "0"),
    ("EXP", "0", "1"),
    ("INKEY", "-1", "0"),
    ("INKEY$", "-1", ""),
    ("INT", "2.5", "2"),
    ("LEN", "\"ABC\"", "3"),
    ("LN", "1", "0"),
    ("LOG", "100", "2"),
    ("RAD", "0", "0"),
    ("SGN", "-4", "-1"),
    ("SIN", "0", "0"),
    ("SQR", "16", "4"),
    ("STR$", "12", "12"),
    ("STR$~", "255", "FF"),
    ("TAN", "0", "0"),
    ("VAL", "\"12\"", "12"),
];

#[test]
fn test_each_function_with_literal_variable_and_bracket() {
    for &(name, literal, expected) in FUNCTIONS {
        let variable = if literal.starts_with('"') { "V$" } else { "V" };
        let assign = format!("10 {variable} = {literal}");
        let with_literal = format!("20 PRINT {name} {literal}");
        let with_variable = format!("30 PRINT {name} {variable}");
        let with_bracket = format!("40 PRINT {name}({literal})");
        let (result, output) = run(&[&assign, &with_literal, &with_variable, &with_bracket]);

        assert_eq!(result, Ok(()), "{name}");
        assert_eq!(
            output,
            format!("{expected}\n{expected}\n{expected}\n"),
            "{name}"
        );
    }
}

#[test]
fn test_function_takes_only_the_next_factor() {
    let (result, output) = run(&["10 X = 0", "20 PRINT SIN X + 1"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n");
}

#[test]
fn test_unary_minus_is_part_of_the_factor() {
    let (result, output) = run(&["10 PRINT ABS -3", "20 PRINT ABS -3 + 1"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "3\n4\n");
}

#[test]
fn test_len_of_variable_plus_one() {
    let (result, output) = run(&["10 A$ = \"AB\"", "20 PRINT LEN A$ + 1"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "3\n");
}

#[test]
fn test_two_string_functions_joined() {
    let (result, output) = run(&["10 PRINT CHR$ 65 + CHR$ 66"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "AB\n");
}

#[test]
fn test_nested_functions_with_no_brackets() {
    let (result, output) = run(&["10 PRINT LEN STR$ 12"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "2\n");
}

#[test]
fn test_int_takes_rnd_and_not_the_product() {
    // INT RND(1) * 6 is (INT RND(1)) * 6. RND(1) is less than 1, so the
    // result is always 0.
    let (result, output) = run(&[
        "10 FOR I% = 1 TO 50",
        "20 IF INT RND(1) * 6 <> 0 THEN PRINT \"BAD\"",
        "30 NEXT I%",
        "40 PRINT \"DONE\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "DONE\n");
}

#[test]
fn test_function_with_no_brackets_in_assignment() {
    let (result, output) = run(&["10 A$ = \"HELLO\"", "20 N% = LEN A$", "30 PRINT N%"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "5\n");
}

#[test]
fn test_function_of_no_value_is_unchanged() {
    let (result, output) = run(&["10 PRINT INT PI"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "3\n");
}

#[test]
fn test_not_stays_an_operator() {
    let (result, output) = run(&["10 A = 0", "20 PRINT NOT A"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "-1\n");
}

#[test]
fn test_function_of_two_values_needs_brackets() {
    for name in ["LEFT$", "MID$", "RIGHT$", "STRING$", "INSTR", "POINT"] {
        let line = format!("20 PRINT {name} A$");
        let (result, _) = run(&["10 A$ = \"ABC\"", &line]);

        assert_eq!(
            result,
            Err(format!(
                "Syntax error at line 20: Expected '(' after {name}"
            )),
            "{name}"
        );
    }
}

#[test]
fn test_function_at_end_of_expression_is_syntax_error() {
    let (result, _) = run(&["10 PRINT LEN"]);

    assert_eq!(
        result,
        Err("Syntax error at line 10: Unexpected end of expression".to_string())
    );
}

#[test]
fn test_variables_with_function_names_plus_letters() {
    let (result, output) = run(&[
        "10 LENGTH = 4 : SINE = 5 : INTEREST = 6",
        "20 PRINT LENGTH; SINE; INTEREST",
        "30 PRINT LEN \"AB\" + LENGTH",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "456\n6\n");
}

#[test]
fn test_arrays_with_function_names_plus_letters() {
    let (result, output) = run(&[
        "10 DIM LENGTH(2), SINE(2), INTEREST(2)",
        "20 LENGTH(1) = 7 : SINE(1) = 8 : INTEREST(1) = 9",
        "30 PRINT LENGTH(1); SINE(1); INTEREST(1)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "789\n");
}

#[test]
fn test_lower_case_function_name_is_a_variable() {
    let (result, output) = run(&["10 len = 5", "20 PRINT len"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "5\n");
}

#[test]
fn test_list_shows_the_line_as_typed() {
    let output = run_binary_read_first("10 PRINT LEN A$\nLIST\nEXIT\n");

    assert!(output.contains("10 PRINT LEN A$\n"), "{output:?}");
}
