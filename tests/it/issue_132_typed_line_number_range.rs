//! Tests that a typed line with a line number above 65535 gives an error, and
//! that the line does not run and is not stored (issue #132).

use crate::common::run_binary_read_first as run;

const OUT_OF_RANGE: &str = "Error: Syntax error: Line number out of range";

#[test]
fn typed_line_above_65535_gives_an_error_and_does_not_run() {
    let output = run("70000 PRINT \"hi\"\nLIST\nEXIT\n");
    assert!(output.contains(OUT_OF_RANGE), "{output}");
    assert!(!output.contains("hi"), "{output}");
    assert!(output.contains("No program"), "{output}");
}

#[test]
fn typed_line_with_a_very_large_number_gives_an_error_and_no_panic() {
    let output = run("99999999999 PRINT 1\nLIST\nPRINT \"alive\"\nEXIT\n");
    assert!(output.contains(OUT_OF_RANGE), "{output}");
    assert!(output.contains("No program"), "{output}");
    assert!(output.contains("alive"), "{output}");
}

#[test]
fn typed_line_number_alone_above_65535_gives_an_error() {
    let output = run("10 PRINT 1\n70000\nLIST\nEXIT\n");
    assert!(output.contains(OUT_OF_RANGE), "{output}");
    assert!(output.contains("10 PRINT 1"), "{output}");
}

#[test]
fn typed_line_65535_is_stored() {
    let output = run("65535 PRINT \"top\"\nLIST\nRUN\nEXIT\n");
    assert!(output.contains("65535 PRINT \"top\""), "{output}");
    assert!(output.contains("top\n"), "{output}");
    assert!(!output.contains("Error"), "{output}");
}
