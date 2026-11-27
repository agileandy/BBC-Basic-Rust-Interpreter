//! Issue #90: PRINT# reads its items with the same reader as PRINT.

use crate::common::run_lines as run;
use std::path::PathBuf;

/// Make an empty temporary directory for one test and return a file path in it.
fn temp_file(name: &str) -> String {
    let dir: PathBuf =
        std::env::temp_dir().join(format!("bbc_issue_90_{}_{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir is created");
    dir.join("data.txt").to_string_lossy().into_owned()
}

/// Run a program that opens the file, runs one PRINT# line and closes the file.
/// Return the bytes of the file.
fn write_with(name: &str, print_line: &str) -> Vec<u8> {
    let file = temp_file(name);
    let open_out = format!("10 F% = OPENOUT(\"{}\")", file);
    let print = format!("20 {}", print_line);
    let (result, _) = run(&[&open_out, &print, "30 CLOSE#F%"]);
    assert_eq!(result, Ok(()));
    std::fs::read(&file).expect("file is readable")
}

#[test]
fn test_print_file_two_numbers_with_no_separator_writes_both() {
    let bytes = write_with("two_numbers", "PRINT#F%, 10 58");

    // Each item is one record (issue #157)
    assert_eq!(String::from_utf8_lossy(&bytes), "10\n58\n");
}

#[test]
fn test_print_file_tan_is_a_function() {
    let bytes = write_with("tan", "PRINT#F%, TAN(0)");

    assert_eq!(String::from_utf8_lossy(&bytes), "0\n");
}

#[test]
fn test_print_file_hex_form() {
    let bytes = write_with("hex", "PRINT#F%, ~255;255");

    // Each item is one record (issue #157)
    assert_eq!(String::from_utf8_lossy(&bytes), "FF\n255\n");
}

#[test]
fn test_print_file_number_comma_string() {
    // This build writes text. Each item is one record (issue #157).
    let bytes = write_with("comma", "PRINT#F%, 1, \"A\"");

    assert_eq!(String::from_utf8_lossy(&bytes), "1\nA\n");
}
