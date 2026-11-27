//! Tests for the JSON path functions JSONGET$, JSONGET, JSONTYPE$, JSONLEN
//! and JSONKEY$ (issue #103).

use crate::common::{run_binary_read_first, run_timed_ran_for as run};

/// The ERR number of "Bad path".
const BAD_PATH_ERR: &str = "152";

/// A nested JSON document.
const DOC: &str = r#"{"user":{"name":"Ann Lee","age":42,"admin":true,"tags":["a","b"]},"items":[{"price":1.5},{"price":2},{"price":"9.25"}],"my key":"spaced","none":null,"off":false}"#;

/// Give a BASIC line that sets `J$` to the text, with each quote doubled.
fn set_json(line: u32, text: &str) -> String {
    format!("{line} J$ = \"{}\"", text.replace('"', "\"\""))
}

#[test]
fn test_jsonget_string_gives_strings_raw_and_other_values_as_json() {
    let (result, output) = run(&[
        &set_json(10, DOC),
        "20 PRINT JSONGET$(J$, \"user.name\")",
        "30 PRINT JSONGET$(J$, \"user.tags\")",
        "40 PRINT JSONGET$(J$, \"items[1]\")",
        "50 PRINT JSONGET$(J$, \"user.tags[1]\")",
        "60 PRINT JSONGET$(J$, \"none\")",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "Ann Lee\n[\"a\",\"b\"]\n{\"price\":2}\nb\nnull\n");
}

#[test]
fn test_jsonget_string_of_empty_path_gives_the_root() {
    let (result, output) = run(&["10 PRINT JSONGET$(\"[1, 2]\", \"\")"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "[1,2]\n");
}

#[test]
fn test_jsonget_gives_numbers() {
    let (result, output) = run(&[
        &set_json(10, DOC),
        "20 PRINT JSONGET(J$, \"user.age\") + 1",
        "30 PRINT JSONGET(J$, \"items[0].price\")",
        "40 PRINT JSONGET(J$, \"items[2].price\")",
        "50 A% = JSONGET(J$, \"items[0].price\")",
        "60 PRINT A%",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "43\n1.5\n9.25\n1\n");
}

#[test]
fn test_jsonget_gives_true_as_minus_1_and_false_and_null_as_0() {
    let (result, output) = run(&[
        &set_json(10, DOC),
        "20 PRINT JSONGET(J$, \"user.admin\")",
        "30 PRINT JSONGET(J$, \"off\")",
        "40 PRINT JSONGET(J$, \"none\")",
        "50 IF JSONGET(J$, \"user.admin\") THEN PRINT \"YES\"",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "-1\n0\n0\nYES\n");
}

#[test]
fn test_jsonget_of_a_string_that_is_not_a_number_gives_type_mismatch() {
    let (result, _) = run(&[&set_json(10, DOC), "20 X = JSONGET(J$, \"user.name\")"]);
    assert_eq!(result, Err("Type mismatch at line 20".to_string()));
}

#[test]
fn test_jsontype_gives_each_type_name() {
    let (result, output) = run(&[
        &set_json(10, DOC),
        "20 PRINT JSONTYPE$(J$, \"\")",
        "30 PRINT JSONTYPE$(J$, \"items\")",
        "40 PRINT JSONTYPE$(J$, \"user.name\")",
        "50 PRINT JSONTYPE$(J$, \"user.age\")",
        "60 PRINT JSONTYPE$(J$, \"user.admin\")",
        "70 PRINT JSONTYPE$(J$, \"none\")",
        "80 PRINT JSONTYPE$(J$, \"user.nick\")",
        "90 PRINT JSONTYPE$(J$, \"items[3]\")",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(
        output,
        "object\narray\nstring\nnumber\nboolean\nnull\nmissing\nmissing\n"
    );
}

#[test]
fn test_jsonlen_gives_array_object_and_string_lengths() {
    let (result, output) = run(&[
        &set_json(10, DOC),
        "20 PRINT JSONLEN(J$, \"items\")",
        "30 PRINT JSONLEN(J$, \"user\")",
        "40 PRINT JSONLEN(J$, \"user.name\")",
        "50 N% = JSONLEN(J$, \"\")",
        "60 PRINT N%",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "3\n4\n7\n5\n");
}

#[test]
fn test_jsonkey_gives_each_key_in_text_order_from_0() {
    let (result, output) = run(&[
        &set_json(10, DOC),
        "20 FOR I% = 0 TO JSONLEN(J$, \"user\") - 1",
        "30 PRINT JSONKEY$(J$, \"user\", I%)",
        "40 NEXT",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "name\nage\nadmin\ntags\n");
}

#[test]
fn test_jsonkey_out_of_range_gives_subscript() {
    let (result, _) = run(&[&set_json(10, DOC), "20 K$ = JSONKEY$(J$, \"user\", 4)"]);
    assert_eq!(result, Err("Subscript at line 20".to_string()));
    let (result, _) = run(&[&set_json(10, DOC), "20 K$ = JSONKEY$(J$, \"user\", -1)"]);
    assert_eq!(result, Err("Subscript at line 20".to_string()));
}

#[test]
fn test_key_with_a_space_through_the_bracket_form() {
    let (result, output) = run(&[
        &set_json(10, DOC),
        "20 P$ = \"[\"\"my key\"\"]\"",
        "30 PRINT JSONGET$(J$, P$)",
        "40 PRINT JSONTYPE$(J$, P$)",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "spaced\nstring\n");
}

#[test]
fn test_missing_path_gives_bad_path_for_4_functions() {
    for call in [
        "A$ = JSONGET$(J$, \"user.nick\")",
        "A = JSONGET(J$, \"user.nick\")",
        "A = JSONLEN(J$, \"user.nick\")",
        "A$ = JSONKEY$(J$, \"user.nick\", 0)",
    ] {
        let (result, _) = run(&[&set_json(10, DOC), &format!("20 {call}")]);
        assert_eq!(
            result,
            Err("Bad path: user.nick at line 20".to_string()),
            "{call}"
        );
    }
}

#[test]
fn test_path_with_wrong_syntax_gives_bad_path() {
    let (result, _) = run(&[&set_json(10, DOC), "20 PRINT JSONTYPE$(J$, \"items[x]\")"]);
    assert_eq!(result, Err("Bad path: items[x] at line 20".to_string()));
}

#[test]
fn test_on_error_catches_bad_path_with_err_152() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 A$ = JSONGET$(\"{}\", \"a\")",
        "30 END",
        "100 PRINT ERR",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), BAD_PATH_ERR);
}

#[test]
fn test_text_that_is_not_json_gives_bad_json() {
    let (result, _) = run(&["10 PRINT JSONTYPE$(\"{bad\", \"\")"]);
    assert_eq!(
        result,
        Err("Bad JSON: key must be a string at line 1 column 2 at line 10".to_string())
    );
}

#[test]
fn test_wrong_argument_count_gives_syntax_error() {
    let (result, _) = run(&["10 PRINT JSONGET$(\"{}\")"]);
    assert_eq!(
        result,
        Err("Syntax error at line 10: JSONGET$ requires 2 arguments".to_string())
    );
    let (result, _) = run(&["10 PRINT JSONKEY$(\"{}\", \"\")"]);
    assert_eq!(
        result,
        Err("Syntax error at line 10: JSONKEY$ requires 3 arguments".to_string())
    );
}

#[test]
fn test_variables_with_the_new_names_still_work() {
    let (result, output) = run(&[
        "10 JSONGET$ = \"a\"",
        "20 JSONGET = 1",
        "30 JSONTYPE$ = \"b\"",
        "40 JSONLEN = 2",
        "50 JSONKEY$ = \"c\"",
        "60 PRINT JSONGET$; JSONGET; JSONTYPE$; JSONLEN; JSONKEY$",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "a1b2c\n");
}

#[test]
fn test_arrays_with_the_new_names_still_work() {
    let (result, output) = run(&[
        "10 DIM JSONGET$(2), JSONGET(2), JSONTYPE$(2), JSONLEN(2), JSONKEY$(2)",
        "20 JSONGET$(1) = \"a\"",
        "30 JSONGET(1) = 1",
        "40 JSONTYPE$(1) = \"b\"",
        "50 JSONLEN(1) = 2",
        "60 JSONKEY$(1) = \"c\"",
        "70 PRINT JSONGET$(1); JSONGET(1); JSONTYPE$(1); JSONLEN(1); JSONKEY$(1)",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "a1b2c\n");
}

#[test]
fn test_binary_runs_a_program_with_the_path_functions() {
    let input = format!(
        "{}\n20 PRINT JSONGET$(J$, \"user.name\")\n30 PRINT JSONGET(J$, \"items[0].price\")\n40 PRINT JSONTYPE$(J$, \"x\")\nRUN\nEXIT\n",
        set_json(10, DOC)
    );
    let output = run_binary_read_first(&input);
    assert!(
        output.contains("Ann Lee\n1.5\nmissing\n"),
        "output: {output}"
    );
}
