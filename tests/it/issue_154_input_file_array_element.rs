//! Issue #154: INPUT# puts a value into an array element. A token that is not
//! a target gives a syntax error.

use crate::common::run_lines as run;
use std::path::PathBuf;

/// Make an empty temporary directory for one test and return a file path in it.
fn temp_file(name: &str) -> String {
    let dir: PathBuf =
        std::env::temp_dir().join(format!("bbc_issue_154_{}_{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir is created");
    dir.join("data.txt").to_string_lossy().into_owned()
}

/// Write the PRINT# line to a new file, then run the read lines from line 40.
/// The read lines can use `H` as the handle of the file.
fn write_then_read(
    name: &str,
    print_line: &str,
    read_lines: &[&str],
) -> (Result<(), String>, String) {
    let file = temp_file(name);
    let open_out = format!("10 H = OPENOUT(\"{}\")", file);
    let print = format!("20 PRINT#H, {}", print_line);
    let open_in = format!("40 H = OPENIN(\"{}\")", file);
    let mut lines: Vec<&str> = vec![&open_out, &print, "30 CLOSE#H", &open_in];
    lines.extend_from_slice(read_lines);
    lines.push("999 CLOSE#H");
    run(&lines)
}

#[test]
fn test_input_file_reads_into_array_elements_and_a_variable() {
    let (result, output) = write_then_read(
        "three_values",
        "7;\",\";\"AB\";\",\";3.5",
        &[
            "50 DIM V(3), N$(3)",
            "60 INPUT#H, V(1), N$(2), X",
            "70 PRINT V(1);\" \";N$(2);\" \";X",
        ],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "7 AB 3.5\n");
}

#[test]
fn test_input_file_reads_into_a_two_dimension_element() {
    let (result, output) = write_then_read(
        "two_dimensions",
        "42",
        &[
            "50 DIM M(2,3)",
            "60 INPUT#H, M(1,2)",
            "70 PRINT M(1,2);\" \";M(2,1)",
        ],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "42 0\n");
}

#[test]
fn test_input_file_reads_into_an_integer_array_element() {
    let (result, output) = write_then_read(
        "integer_element",
        "12",
        &["50 DIM V%(2)", "60 INPUT#H, V%(2)", "70 PRINT V%(2)"],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "12\n");
}

#[test]
fn test_input_file_evaluates_the_indices_when_the_target_gets_its_value() {
    // D3: I% gets 2 before the index of V(I%) is evaluated
    let (result, output) = write_then_read(
        "index_order",
        "\"2,9\"",
        &[
            "50 DIM V(3)",
            "60 I% = 0",
            "70 INPUT#H, I%, V(I%)",
            "80 PRINT V(0);\" \";V(2)",
        ],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0 9\n");
}

#[test]
fn test_input_file_variables_are_unchanged() {
    let (result, output) = write_then_read(
        "variables",
        "\"5,HELLO\"",
        &["60 INPUT#H, A, B$", "70 PRINT A;\" \";B$"],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "5 HELLO\n");
}

#[test]
fn test_input_file_text_after_an_element_gives_a_syntax_error() {
    let (result, _) = run(&["10 DIM V(3)", "20 INPUT#H, V(1) X"]);

    assert_eq!(
        result,
        Err(
            "Syntax error at line 20: Expected a variable or an array element in INPUT#"
                .to_string()
        )
    );
}

#[test]
fn test_input_file_text_after_a_variable_gives_a_syntax_error() {
    let (result, _) = run(&["10 INPUT#H, A B"]);

    assert_eq!(
        result,
        Err(
            "Syntax error at line 10: Expected a variable or an array element in INPUT#"
                .to_string()
        )
    );
}

#[test]
fn test_input_file_a_semicolon_between_targets_gives_a_syntax_error() {
    let (result, _) = run(&["10 INPUT#H, A; B"]);

    assert_eq!(
        result,
        Err(
            "Syntax error at line 10: Expected a variable or an array element in INPUT#"
                .to_string()
        )
    );
}

#[test]
fn test_input_file_an_empty_target_gives_a_syntax_error() {
    let (result, _) = run(&["10 INPUT#H, A,"]);

    assert_eq!(
        result,
        Err(
            "Syntax error at line 10: Expected a variable or an array element in INPUT#"
                .to_string()
        )
    );
}

#[test]
fn test_bget_into_an_array_element() {
    let (result, output) = write_then_read(
        "bget",
        "\"A\"",
        &["50 DIM V(2)", "60 V(1) = BGET#H", "70 PRINT V(1)"],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "65\n");
}
