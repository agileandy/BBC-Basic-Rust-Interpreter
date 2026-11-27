//! Tests for the JSON change functions JSONSET$, JSONSETJSON$, JSONADD$ and
//! JSONDEL$ (issue #104).

use crate::common::{run_binary_read_first, run_timed_ran_for as run};

#[test]
fn test_build_a_document_from_an_empty_string_in_3_calls() {
    let (result, output) = run(&[
        "10 J$ = JSONSET$(\"\", \"name\", \"A\")",
        "20 J$ = JSONADD$(J$, \"items\", 1)",
        "30 J$ = JSONADD$(J$, \"items\", 2)",
        "40 PRINT J$",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "{\"name\":\"A\",\"items\":[1,2]}\n");
}

#[test]
fn test_jsonset_changes_a_value_and_does_not_change_its_argument() {
    let (result, output) = run(&[
        "10 J$ = \"{\"\"a\"\":1,\"\"b\"\":2}\"",
        "20 K$ = JSONSET$(J$, \"a\", \"x\")",
        "30 PRINT K$",
        "40 PRINT J$",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "{\"a\":\"x\",\"b\":2}\n{\"a\":1,\"b\":2}\n");
}

#[test]
fn test_jsonset_gives_a_number_for_a_numeric_expression() {
    let (result, output) = run(&[
        "10 A% = 7",
        "20 J$ = JSONSET$(\"\", \"i\", A%)",
        "30 J$ = JSONSET$(J$, \"r\", 2.5 * 2)",
        "40 J$ = JSONSET$(J$, \"f\", 1.5)",
        "50 J$ = JSONSET$(J$, \"s\", STR$(3))",
        "60 PRINT J$",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "{\"i\":7,\"r\":5,\"f\":1.5,\"s\":\"3\"}\n");
}

#[test]
fn test_jsonset_makes_missing_parent_objects_and_appends_at_the_length() {
    let (result, output) = run(&[
        "10 PRINT JSONSET$(\"\", \"a.b[\"\"c d\"\"]\", 1)",
        "20 PRINT JSONSET$(\"[1,2]\", \"[2]\", 3)",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "{\"a\":{\"b\":{\"c d\":1}}}\n[1,2,3]\n");
}

#[test]
fn test_jsonset_escapes_a_quote_and_a_new_line() {
    let (result, output) = run(&[
        "10 S$ = \"say \"\"hi\"\"\" + CHR$(10) + \"bye\"",
        "20 J$ = JSONSET$(\"\", \"s\", S$)",
        "30 PRINT J$",
        "40 PRINT JSONGET$(J$, \"s\") = S$",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "{\"s\":\"say \\\"hi\\\"\\nbye\"}\n-1\n");
}

#[test]
fn test_jsonsetjson_puts_an_object_an_array_true_false_and_null() {
    let (result, output) = run(&[
        "10 J$ = JSONSETJSON$(\"\", \"o\", \"{\"\"x\"\": [1, 2]}\")",
        "20 J$ = JSONSETJSON$(J$, \"t\", \"true\")",
        "30 J$ = JSONSETJSON$(J$, \"f\", \"false\")",
        "40 J$ = JSONSETJSON$(J$, \"n\", \"null\")",
        "50 PRINT J$",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(
        output,
        "{\"o\":{\"x\":[1,2]},\"t\":true,\"f\":false,\"n\":null}\n"
    );
}

#[test]
fn test_jsonadd_appends_a_string_and_a_nested_array() {
    let (result, output) = run(&[
        "10 J$ = JSONADD$(\"[]\", \"\", \"x\")",
        "20 J$ = JSONADD$(J$, \"\", 2)",
        "30 PRINT J$",
        "40 PRINT JSONADD$(\"\", \"a.list\", 1)",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "[\"x\",2]\n{\"a\":{\"list\":[1]}}\n");
}

#[test]
fn test_jsondel_removes_a_key_and_an_element() {
    let (result, output) = run(&[
        "10 J$ = \"{\"\"a\"\":1,\"\"b\"\":[1,2,3],\"\"c\"\":3}\"",
        "20 J$ = JSONDEL$(J$, \"a\")",
        "30 J$ = JSONDEL$(J$, \"b[1]\")",
        "40 PRINT J$",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "{\"b\":[1,3],\"c\":3}\n");
}

#[test]
fn test_each_error_text() {
    for (call, error) in [
        (
            "J$ = JSONSETJSON$(\"\", \"a\", \"[1,\")",
            "Bad JSON: EOF while parsing a value at line 1 column 3",
        ),
        ("J$ = JSONDEL$(\"{}\", \"a\")", "Bad path: a"),
        ("J$ = JSONSET$(\"[]\", \"[1]\", 1)", "Bad path: [1]"),
        (
            "J$ = JSONADD$(\"{\"\"a\"\":1}\", \"a\", 2)",
            "Type mismatch",
        ),
        (
            "J$ = JSONSET$(\"{bad\", \"a\", 1)",
            "Bad JSON: key must be a string at line 1 column 2",
        ),
        (
            "J$ = JSONSET$(\"\", \"a\", STRING$(65530, \"x\"))",
            "String too long",
        ),
    ] {
        let (result, _) = run(&[&format!("10 {call}")]);
        assert_eq!(result, Err(format!("{error} at line 10")), "{call}");
    }
}

#[test]
fn test_on_error_catches_each_error_with_its_err_number() {
    for (call, err) in [
        ("J$ = JSONSETJSON$(\"\", \"a\", \"nope\")", "150"),
        ("J$ = JSONDEL$(\"{}\", \"a\")", "152"),
        ("J$ = JSONADD$(\"{\"\"a\"\":1}\", \"a\", 2)", "6"),
        ("J$ = JSONSET$(\"\", \"a\", STRING$(65530, \"x\"))", "19"),
    ] {
        let (result, output) = run(&[
            "10 ON ERROR GOTO 100",
            &format!("20 {call}"),
            "30 END",
            "100 PRINT ERR",
        ]);
        assert_eq!(result, Ok(()), "{call}");
        assert_eq!(output.trim(), err, "{call}");
    }
}

#[test]
fn test_wrong_argument_count_gives_syntax_error() {
    for (call, name, count) in [
        ("JSONSET$(\"\", \"a\")", "JSONSET$", 3),
        ("JSONSETJSON$(\"\", \"a\")", "JSONSETJSON$", 3),
        ("JSONADD$(\"\")", "JSONADD$", 3),
        ("JSONDEL$(\"\")", "JSONDEL$", 2),
    ] {
        let (result, _) = run(&[&format!("10 PRINT {call}")]);
        assert_eq!(
            result,
            Err(format!(
                "Syntax error at line 10: {name} requires {count} arguments"
            )),
            "{call}"
        );
    }
}

#[test]
fn test_variables_with_the_new_names_still_work() {
    let (result, output) = run(&[
        "10 JSONSET$ = \"a\"",
        "20 JSONSETJSON$ = \"b\"",
        "30 JSONADD$ = \"c\"",
        "40 JSONDEL$ = \"d\"",
        "50 PRINT JSONSET$; JSONSETJSON$; JSONADD$; JSONDEL$",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "abcd\n");
}

#[test]
fn test_arrays_with_the_new_names_still_work() {
    let (result, output) = run(&[
        "10 DIM JSONSET$(2), JSONSETJSON$(2), JSONADD$(2), JSONDEL$(2)",
        "20 JSONSET$(1) = \"a\"",
        "30 JSONSETJSON$(1) = \"b\"",
        "40 JSONADD$(1) = \"c\"",
        "50 JSONDEL$(1) = \"d\"",
        "60 PRINT JSONSET$(1); JSONSETJSON$(1); JSONADD$(1); JSONDEL$(1)",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "abcd\n");
}

#[test]
fn test_binary_runs_a_program_with_the_change_functions() {
    let input = "10 J$ = JSONSET$(\"\", \"name\", \"A\")\n20 J$ = JSONADD$(J$, \"items\", 1)\n30 J$ = JSONADD$(J$, \"items\", 2)\n40 PRINT JSONDEL$(J$, \"name\")\nRUN\nEXIT\n";
    let output = run_binary_read_first(input);
    assert!(output.contains("{\"items\":[1,2]}\n"), "output: {output}");
}
