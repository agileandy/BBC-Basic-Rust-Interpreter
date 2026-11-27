//! Tests that DELETE start, end removes a range of lines (issue #32).

use crate::common::run_binary_write_first as run_binary;

const PROGRAM: &str = "10 PRINT \"ONE\"\n20 PRINT \"TWO\"\n30 PRINT \"THREE\"\n40 PRINT \"FOUR\"\n50 PRINT \"FIVE\"\n";
const ALL_TEXT: [&str; 5] = ["ONE", "TWO", "THREE", "FOUR", "FIVE"];

#[test]
fn test_delete_range_removes_lines_from_start_to_end() {
    let output = run_binary(&format!("{PROGRAM}DELETE 20, 40\nLIST\nEXIT\n"));

    assert!(output.contains("10 PRINT \"ONE\"\n"), "output: {output}");
    assert!(output.contains("50 PRINT \"FIVE\"\n"), "output: {output}");
    assert!(!output.contains("TWO"), "output: {output}");
    assert!(!output.contains("THREE"), "output: {output}");
    assert!(!output.contains("FOUR"), "output: {output}");
    assert!(!output.contains("Error"), "output: {output}");
}

#[test]
fn test_delete_range_with_no_lines_gives_no_error() {
    let output = run_binary(&format!("{PROGRAM}DELETE 21, 29\nLIST\nEXIT\n"));

    assert!(!output.contains("Error"), "output: {output}");
    for text in ALL_TEXT {
        assert!(output.contains(text), "missing {text}, output: {output}");
    }
}

#[test]
fn test_delete_range_in_wrong_order_gives_error_and_removes_nothing() {
    let output = run_binary(&format!("{PROGRAM}DELETE 40, 20\nLIST\nEXIT\n"));

    assert!(output.contains("Error: Syntax error"), "output: {output}");
    for text in ALL_TEXT {
        assert!(output.contains(text), "missing {text}, output: {output}");
    }
}

#[test]
fn test_delete_with_one_number_gives_error_and_removes_nothing() {
    let output = run_binary(&format!("{PROGRAM}DELETE 20\nLIST\nEXIT\n"));

    assert!(output.contains("Error: Syntax error"), "output: {output}");
    for text in ALL_TEXT {
        assert!(output.contains(text), "missing {text}, output: {output}");
    }
}

#[test]
fn test_delete_in_lower_case_removes_the_range() {
    let output = run_binary(&format!("{PROGRAM}delete 10,20\nLIST\nEXIT\n"));

    assert!(!output.contains("ONE"), "output: {output}");
    assert!(!output.contains("TWO"), "output: {output}");
    assert!(output.contains("30 PRINT \"THREE\"\n"), "output: {output}");
    assert!(!output.contains("Error"), "output: {output}");
}
