//! Issue #162: SWAP exchanges the values of 2 variables or array elements of
//! the same type class.

use crate::common::run_with_limit as run;
use bbc_basic_interpreter::tokenizer::{detokenize, tokenize};

#[test]
fn test_swap_2_reals() {
    let (result, output) = run(&[
        "10 A = 1.5 : B = 2.25",
        "20 SWAP A, B",
        "30 PRINT A; \" \"; B",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "      2.25 1.5\n");
}

#[test]
fn test_swap_2_integers() {
    let (result, output) = run(&[
        "10 A% = 3 : B% = 7",
        "20 SWAP A%, B%",
        "30 PRINT A%; \" \"; B%",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "         7 3\n");
}

#[test]
fn test_swap_a_resident_integer_with_no_assignment() {
    let (result, output) = run(&["10 X% = 5", "20 SWAP Q%, X%", "30 PRINT Q%; \" \"; X%"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "         5 0\n");
}

#[test]
fn test_swap_2_strings() {
    let (result, output) = run(&[
        "10 A$ = \"left\" : B$ = \"right\"",
        "20 SWAP A$, B$",
        "30 PRINT A$; \" \"; B$",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "right left\n");
}

#[test]
fn test_swap_2_array_elements() {
    let (result, output) = run(&[
        "10 DIM N$(3), Q(2, 2)",
        "20 N$(1) = \"x\" : N$(3) = \"z\" : Q(1, 2) = 4 : Q(2, 1) = 9",
        "30 SWAP N$(1), N$(3) : SWAP Q(1, 2), Q(2, 1)",
        "40 PRINT N$(1); N$(3); \" \"; Q(1, 2); \" \"; Q(2, 1)",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "zx 9 4\n");
}

#[test]
fn test_swap_an_element_and_a_variable() {
    let (result, output) = run(&[
        "10 DIM A%(2)",
        "20 A%(2) = 5 : B% = 8",
        "30 SWAP A%(2), B%",
        "40 PRINT A%(2); \" \"; B%",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "         8 5\n");
}

#[test]
fn test_indices_are_evaluated_one_time_before_the_exchange() {
    let (result, output) = run(&[
        "10 DIM A(3)",
        "20 I = 1 : A(1) = 3 : A(3) = 30",
        "30 SWAP I, A(I)",
        "40 PRINT I; \" \"; A(1); \" \"; A(3)",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "         3 1 30\n");
}

#[test]
fn test_swap_elements_in_a_bubble_sort() {
    let (result, output) = run(&[
        "10 DIM A(5)",
        "20 FOR I = 1 TO 5 : READ A(I) : NEXT",
        "30 FOR I = 1 TO 4",
        "40 FOR J = 1 TO 5 - I",
        "50 IF A(J) > A(J + 1) THEN SWAP A(J), A(J + 1)",
        "60 NEXT : NEXT",
        "70 FOR I = 1 TO 5 : PRINT A(I); \" \"; : NEXT : PRINT",
        "80 DATA 4, 1, 5, 2, 3",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(
        output,
        "         1          2          3          4          5 \n"
    );
}

#[test]
fn test_swap_an_integer_and_a_real_converts_the_values() {
    let (result, output) = run(&[
        "10 A% = 2 : B = 7.8",
        "20 SWAP A%, B",
        "30 PRINT A%; \" \"; B",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "         7 2\n");
}

#[test]
fn test_a_real_that_does_not_fit_gives_too_big_and_changes_nothing() {
    let (result, output) = run(&[
        "10 ON ERROR PRINT REPORT$; \" \"; A%; \" \"; B : END",
        "20 A% = 2 : B = 1E20",
        "30 SWAP A%, B",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "Too big 2 1E20\n");
}

#[test]
fn test_too_big_error_text() {
    let (result, _) = run(&["10 A% = 2 : B = 1E20", "20 SWAP B, A%"]);
    assert_eq!(result, Err("Too big at line 20".to_string()));
}

#[test]
fn test_a_number_and_a_string_give_type_mismatch_and_change_nothing() {
    let (result, output) = run(&[
        "10 ON ERROR PRINT REPORT$; \" \"; A; \" \"; B$ : END",
        "20 A = 1 : B$ = \"b\"",
        "30 SWAP A, B$",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "Type mismatch 1 b\n");
}

#[test]
fn test_type_mismatch_error_text() {
    let (result, _) = run(&["10 A$ = \"a\" : B% = 1", "20 SWAP A$, B%"]);
    assert_eq!(result, Err("Type mismatch at line 20".to_string()));
}

#[test]
fn test_swap_with_one_target_is_a_syntax_error() {
    let (result, _) = run(&["10 A = 1", "20 SWAP A"]);
    assert_eq!(
        result,
        Err("Syntax error at line 20: SWAP requires 2 variables".to_string())
    );
}

#[test]
fn test_swap_with_3_targets_is_a_syntax_error() {
    let (result, _) = run(&["10 SWAP A, B, C"]);
    assert_eq!(
        result,
        Err("Syntax error at line 10: SWAP requires 2 variables".to_string())
    );
}

#[test]
fn test_swap_of_whole_arrays_is_a_syntax_error() {
    let (result, _) = run(&["10 DIM A(2), B(2)", "20 SWAP A(), B()"]);
    assert_eq!(
        result,
        Err("Syntax error at line 20: Expected variable name in SWAP".to_string())
    );
}

#[test]
fn test_swap_of_an_expression_is_a_syntax_error() {
    let (result, _) = run(&["10 SWAP A, 2"]);
    assert_eq!(
        result,
        Err("Syntax error at line 10: Expected variable name in SWAP".to_string())
    );
}

#[test]
fn test_swap_of_an_unknown_variable_gives_no_such_variable() {
    let (result, _) = run(&["10 A = 1", "20 SWAP A, B"]);
    assert_eq!(result, Err("No such variable: B at line 20".to_string()));
}

#[test]
fn test_lower_case_swap_is_a_variable_and_an_array_name() {
    let (result, output) = run(&[
        "10 swap = 4 : DIM swap(2)",
        "20 swap(1) = swap + 1",
        "30 PRINT swap; \" \"; swap(1)",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "         4 5\n");
}

#[test]
fn test_swap_lists_as_typed() {
    let source = "20 SWAP A(J),A(J + 1)";
    let line = tokenize(source).expect("line tokenizes");
    assert_eq!(detokenize(&line).expect("line detokenizes"), source);
}
