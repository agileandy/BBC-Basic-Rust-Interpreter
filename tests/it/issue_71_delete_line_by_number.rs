//! Tests that a line number typed alone deletes that line (issue #71).

use crate::common::run_binary_write_first as run_binary;

const PROGRAM: &str = "10 PRINT \"ONE\"\n20 PRINT \"TWO\"\n30 PRINT \"THREE\"\n";

#[test]
fn test_line_number_alone_deletes_the_line_from_list() {
    let output = run_binary(&format!("{PROGRAM}20\nLIST\nEXIT\n"));

    assert!(output.contains("10 PRINT \"ONE\"\n"), "output: {output}");
    assert!(output.contains("30 PRINT \"THREE\"\n"), "output: {output}");
    assert!(!output.contains("TWO"), "output: {output}");
    assert!(!output.contains("Error"), "output: {output}");
}

#[test]
fn test_line_number_alone_deletes_the_line_from_run() {
    let output = run_binary(&format!("{PROGRAM}20\nRUN\nEXIT\n"));

    assert!(output.contains("ONE\n"), "output: {output}");
    assert!(output.contains("THREE\n"), "output: {output}");
    assert!(!output.contains("TWO"), "output: {output}");
}

#[test]
fn test_line_number_with_spaces_deletes_the_line() {
    let output = run_binary(&format!("{PROGRAM}  20   \nLIST\nEXIT\n"));

    assert!(output.contains("10 PRINT \"ONE\"\n"), "output: {output}");
    assert!(output.contains("30 PRINT \"THREE\"\n"), "output: {output}");
    assert!(!output.contains("TWO"), "output: {output}");
}

#[test]
fn test_number_of_missing_line_gives_no_error_text() {
    let output = run_binary(&format!("{PROGRAM}25\nLIST\nEXIT\n"));

    assert!(!output.contains("Error"), "output: {output}");
    assert!(!output.contains("deleted"), "output: {output}");
    assert!(output.contains("20 PRINT \"TWO\"\n"), "output: {output}");
}
