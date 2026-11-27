//! Issue #157: PRINT# writes each value as one record (one line), and INPUT#
//! reads one record for each target.

use crate::common::{run_binary_read_first, run_lines as run};
use std::path::PathBuf;

/// Make an empty temporary directory for one test and return a file path in it.
fn temp_file(name: &str) -> String {
    let dir: PathBuf =
        std::env::temp_dir().join(format!("bbc_issue_157_{}_{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir is created");
    dir.join("data.txt").to_string_lossy().into_owned()
}

/// Run the write lines from line 20 with `H` open for output, close the file,
/// then run the read lines from line 50 with `H` open for input.
/// Give the result, the output and the bytes of the file.
fn write_then_read(
    name: &str,
    write_lines: &[&str],
    read_lines: &[&str],
) -> (Result<(), String>, String, String) {
    let file = temp_file(name);
    let open_out = format!("10 H = OPENOUT(\"{}\")", file);
    let open_in = format!("40 H = OPENIN(\"{}\")", file);
    let mut lines: Vec<&str> = vec![&open_out];
    lines.extend_from_slice(write_lines);
    lines.push("30 CLOSE#H");
    lines.push(&open_in);
    lines.extend_from_slice(read_lines);
    lines.push("999 CLOSE#H");
    let (result, output) = run(&lines);
    let bytes = std::fs::read(&file).expect("file is readable");
    (result, output, String::from_utf8_lossy(&bytes).into_owned())
}

#[test]
fn test_three_values_come_back_with_one_input_file() {
    let (result, output, file) = write_then_read(
        "three_values",
        &["20 PRINT#H, 7, \"AB\", 3.5"],
        &["50 INPUT#H, A, B$, C", "60 PRINT A;\" \";B$;\" \";C"],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(file, "7\nAB\n3.5\n");
    assert_eq!(output, "         7 AB 3.5\n");
}

#[test]
fn test_string_with_comma_and_leading_spaces_comes_back_the_same() {
    let (result, output, _) = write_then_read(
        "comma_spaces",
        &["20 PRINT#H, \"  a, b  \""],
        &["50 INPUT#H, S$", "60 PRINT \"[\";S$;\"]\""],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "[  a, b  ]\n");
}

#[test]
fn test_numbers_come_back_with_all_digits() {
    let (result, output, _) = write_then_read(
        "digits",
        &["20 PRINT#H, 0.1, 1/3, 1E30, -2147483648"],
        &[
            "50 INPUT#H, A, B, C, D",
            "60 PRINT A = 0.1; B = 1/3; C = 1E30; D = -2147483648",
        ],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "        -1-1-1-1\n");
}

#[test]
fn test_semicolon_gives_the_same_file_as_comma() {
    let (_, _, comma) = write_then_read("with_comma", &["20 PRINT#H, 7, \"AB\", 3.5"], &[]);
    let (result, _, semicolon) =
        write_then_read("with_semicolon", &["20 PRINT#H, 7; \"AB\"; 3.5;"], &[]);

    assert_eq!(result, Ok(()));
    assert_eq!(semicolon, comma);
}

#[test]
fn test_three_print_file_statements_and_one_input_file() {
    let (result, output, _) = write_then_read(
        "three_statements",
        &["20 PRINT#H, 1", "21 PRINT#H, \"two\"", "22 PRINT#H, 3"],
        &["50 INPUT#H, A%, B$, C", "60 PRINT A%;\" \";B$;\" \";C"],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         1 two 3\n");
}

#[test]
fn test_read_past_the_end_gives_the_eof_error() {
    let (result, _, _) =
        write_then_read("past_end", &["20 PRINT#H, 1, 2"], &["50 INPUT#H, A, B, C"]);

    assert_eq!(result, Err("EOF at line 50".to_string()));
}

#[test]
fn test_on_error_catches_the_eof_error() {
    let (result, output, _) = write_then_read(
        "past_end_caught",
        &["20 PRINT#H, 1"],
        &[
            "45 ON ERROR PRINT ERR; \" \"; ERL : CLOSE#H : END",
            "50 INPUT#H, A, B",
        ],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "       223 50\n");
}

#[test]
fn test_eof_is_true_after_the_last_value() {
    let (result, output, _) = write_then_read(
        "eof_after_last",
        &["20 PRINT#H, 1, 2"],
        &[
            "50 INPUT#H, A",
            "60 PRINT EOF#H",
            "70 INPUT#H, B",
            "80 PRINT EOF#H",
        ],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         0\n        -1\n");
}

#[test]
fn test_array_elements_are_targets() {
    let (result, output, _) = write_then_read(
        "array_targets",
        &["20 PRINT#H, 7, \"AB\", 3.5"],
        &[
            "50 DIM V(3), N$(3)",
            "60 INPUT#H, V(1), N$(2), V(3)",
            "70 PRINT V(1);\" \";N$(2);\" \";V(3)",
        ],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         7 AB 3.5\n");
}

#[test]
fn test_string_with_a_line_feed_gives_an_error_and_writes_nothing() {
    let (result, _, file) = write_then_read(
        "line_feed",
        &["20 PRINT#H, 1, \"A\" + CHR$(10) + \"B\""],
        &[],
    );

    assert_eq!(result, Err("Bad record at line 20".to_string()));
    assert_eq!(file, "");
}

#[test]
fn test_string_with_a_carriage_return_gives_an_error() {
    let (result, output, _) = write_then_read(
        "carriage_return",
        &[
            "15 ON ERROR PRINT ERR : CLOSE#H : END",
            "20 PRINT#H, \"A\" + CHR$(13)",
        ],
        &[],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "       155\n");
}

#[test]
fn test_print_file_with_no_item_writes_an_empty_record() {
    // `PRINT#H` with no comma is a syntax error, so the test uses `PRINT#H,`
    let (result, output, file) = write_then_read(
        "no_item",
        &["20 PRINT#H,", "21 PRINT#H, \"X\""],
        &["50 INPUT#H, A$, B$", "60 PRINT \"[\";A$;\"][\";B$;\"]\""],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(file, "\nX\n");
    assert_eq!(output, "[][X]\n");
}

#[test]
fn test_last_line_with_no_line_feed_is_one_record() {
    let file = temp_file("no_final_line_feed");
    std::fs::write(&file, "5\nlast").expect("file is written");
    let open_in = format!("10 H = OPENIN(\"{}\")", file);
    let (result, output) = run(&[
        &open_in,
        "20 INPUT#H, A, B$",
        "30 PRINT A;\" \";B$;\" \";EOF#H",
        "40 CLOSE#H",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         5 last -1\n");
}

#[test]
fn test_old_file_with_tabs_reads_the_full_line_into_a_string() {
    let file = temp_file("old_tabs");
    std::fs::write(&file, "7\tAB\t3.5\n").expect("file is written");
    let open_in = format!("10 H = OPENIN(\"{}\")", file);
    let (result, output) = run(&[
        &open_in,
        "20 INPUT#H, S$",
        "30 PRINT S$ = \"7\" + CHR$(9) + \"AB\" + CHR$(9) + \"3.5\"",
        "40 CLOSE#H",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "        -1\n");
}

#[test]
fn test_real_binary_writes_and_reads_records() {
    let file = temp_file("binary");
    let input = format!(
        "10 H = OPENOUT \"{0}\" : PRINT#H, 7, \"AB\", 3.5 : CLOSE#H\n\
         20 H = OPENIN \"{0}\" : INPUT#H, A, B$, C : CLOSE#H\n\
         30 PRINT A;\" \";B$;\" \";C\n\
         RUN\nEXIT\n",
        file
    );

    let output = run_binary_read_first(&input);

    assert!(output.contains("7 AB 3.5\n"), "output: {}", output);
}
