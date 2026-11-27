//! Issue 177: after the closing bracket of an array, and after the size of a
//! memory block, DIM accepts only a comma or the end of the statement. Other
//! text gives a syntax error.

use crate::common::run_lines;

#[test]
fn test_dim_with_no_comma_between_2_arrays_gives_a_syntax_error() {
    let (result, output) = run_lines(&["10 DIM A(1) B(2) : PRINT \"ran\""]);

    assert_eq!(
        result,
        Err("Syntax error at line 10: Unexpected B after DIM item".to_string())
    );
    assert!(!output.contains("ran"));
}

#[test]
fn test_dim_with_a_number_after_an_array_gives_a_syntax_error() {
    let (result, _) = run_lines(&["10 DIM A(1) 5"]);

    assert_eq!(
        result,
        Err("Syntax error at line 10: Unexpected 5 after DIM item".to_string())
    );
}

#[test]
fn test_dim_block_with_2_sizes_gives_a_syntax_error() {
    let (result, output) = run_lines(&["10 DIM P% 10 20 : PRINT \"ran\""]);

    assert_eq!(
        result,
        Err("Syntax error at line 10: Unexpected 20 after expression".to_string())
    );
    assert!(!output.contains("ran"));
}

#[test]
fn test_dim_with_a_comma_between_2_arrays_still_works() {
    let (result, output) = run_lines(&["10 DIM A(1), B(2)", "20 B(2) = 7", "30 PRINT A(1); B(2)"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "07");
}

#[test]
fn test_dim_with_a_comma_between_2_blocks_still_works() {
    let (result, output) = run_lines(&["10 DIM P% 10, Q% 20", "20 PRINT Q% - P% > 0"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "-1");
}

#[test]
fn test_dim_followed_by_a_colon_and_a_statement_still_works() {
    let (result, output) = run_lines(&["10 DIM A(1) : PRINT \"ran\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "ran");
}
