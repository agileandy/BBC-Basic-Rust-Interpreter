//! Issue #189: the dot operator `A() = B() . C()`, the matrix product of BBC
//! BASIC for Windows. The manual page is bbcwin2.html, "Dot product". The
//! errors are from the function `expra` of BBC BASIC for SDL (src/bbeval.c).

use crate::common::run_binary_read_first;
use crate::common::run_with_limit as run;
use bbc_basic_interpreter::tokenizer::{detokenize, tokenize};

/// Run the program and give the output, or panic with the error.
fn output_of(source: &[&str]) -> String {
    let (result, output) = run(source);
    assert_eq!(result, Ok(()), "output: {output}");
    output
}

/// Run the program and give the error text
fn error_of(source: &[&str]) -> String {
    let (result, output) = run(source);
    result.expect_err(&format!("the program fails, output: {output}"))
}

/// The lines that set M() to the 2 by 3 matrix 1 2 3 / 4 5 6
const MATRIX_2_BY_3: [&str; 2] = ["10 DIM M(1, 2)", "20 M() = 1, 2, 3, 4, 5, 6"];

// The 4 cases of the manual

#[test]
fn test_a_2_by_3_matrix_and_a_3_by_2_matrix() {
    let output = output_of(&[
        MATRIX_2_BY_3[0],
        MATRIX_2_BY_3[1],
        "30 DIM N(2, 1), R(1, 1)",
        "40 N() = 7, 8, 9, 10, 11, 12",
        "50 R() = M() . N()",
        "60 PRINT ;R(0, 0);\" \";R(0, 1);\" \";R(1, 0);\" \";R(1, 1)",
    ]);
    assert_eq!(output, "58 64 139 154\n");
}

#[test]
fn test_a_matrix_and_a_vector_gives_a_column() {
    let output = output_of(&[
        MATRIX_2_BY_3[0],
        MATRIX_2_BY_3[1],
        "30 DIM V(2), R(1)",
        "40 V() = 1, 2, 3",
        "50 R() = M() . V()",
        "60 PRINT ;R(0);\" \";R(1)",
    ]);
    assert_eq!(output, "14 32\n");
}

#[test]
fn test_a_vector_and_a_matrix_gives_a_row() {
    let output = output_of(&[
        MATRIX_2_BY_3[0],
        MATRIX_2_BY_3[1],
        "30 DIM V(1), R(2)",
        "40 V() = 1, 2",
        "50 R() = V() . M()",
        "60 PRINT ;R(0);\" \";R(1);\" \";R(2)",
    ]);
    assert_eq!(output, "9 12 15\n");
}

#[test]
fn test_2_vectors_give_the_dot_product_in_an_array_of_1_element() {
    let output = output_of(&[
        "10 DIM U(2), V(2), R(0)",
        "20 U() = 1, 2, 3",
        "30 V() = 4, 5, 6",
        "40 R() = U() . V()",
        "50 PRINT ;R(0)",
    ]);
    assert_eq!(output, "32\n");
}

#[test]
fn test_the_target_needs_only_the_number_of_elements() {
    // The manual: the total number of elements must be rows times columns
    let output = output_of(&[
        MATRIX_2_BY_3[0],
        MATRIX_2_BY_3[1],
        "30 DIM N(2, 1), R(3)",
        "40 N() = 7, 8, 9, 10, 11, 12",
        "50 R() = M() . N()",
        "60 PRINT ;R(0);\" \";R(1);\" \";R(2);\" \";R(3)",
    ]);
    assert_eq!(output, "58 64 139 154\n");
}

#[test]
fn test_the_identity_matrix_gives_the_same_matrix() {
    let output = output_of(&[
        "10 DIM I(2, 2), M(2, 2), R(2, 2)",
        "20 I() = 1, 0, 0, 0, 1, 0, 0, 0, 1",
        "30 M() = 1, 2, 3, 4, 5, 6, 7, 8, 9",
        "40 R() = I() . M()",
        "50 FOR J% = 0 TO 2 : FOR K% = 0 TO 2 : PRINT ;R(J%, K%);\" \"; : NEXT : NEXT",
        "60 PRINT",
    ]);
    assert_eq!(output, "1 2 3 4 5 6 7 8 9 \n");
}

#[test]
fn test_spaces_are_optional() {
    let output = output_of(&[
        "10 DIM U(1), V(1), R(0)",
        "20 U() = 2, 3",
        "30 V() = 4, 5",
        "40 R()=U().V()",
        "50 PRINT ;R(0)",
    ]);
    assert_eq!(output, "23\n");
}

// Sizes that do not match

#[test]
fn test_columns_of_the_first_not_the_rows_of_the_second_is_a_type_mismatch() {
    let error = error_of(&[
        MATRIX_2_BY_3[0],
        MATRIX_2_BY_3[1],
        "30 DIM N(1, 1), R(1, 1)",
        "40 R() = M() . N()",
    ]);
    assert_eq!(error, "Type mismatch at line 40");
}

#[test]
fn test_a_target_of_the_wrong_size_is_a_type_mismatch() {
    let error = error_of(&[
        MATRIX_2_BY_3[0],
        MATRIX_2_BY_3[1],
        "30 DIM N(2, 1), R(2, 2)",
        "40 R() = M() . N()",
    ]);
    assert_eq!(error, "Type mismatch at line 40");
}

#[test]
fn test_vectors_of_different_lengths_are_a_type_mismatch() {
    let error = error_of(&["10 DIM U(2), V(3), R(0)", "20 R() = U() . V()"]);
    assert_eq!(error, "Type mismatch at line 20");
}

#[test]
fn test_an_array_of_3_dimensions_is_a_type_mismatch() {
    let error = error_of(&["10 DIM T(1, 1, 1), V(1), R(3)", "20 R() = T() . V()"]);
    assert_eq!(error, "Type mismatch at line 20");
}

#[test]
fn test_an_error_leaves_the_target_unchanged() {
    let (result, output) = run(&[
        "10 DIM U(2), V(3), R(0)",
        "20 R() = 7",
        "30 ON ERROR PRINT ;R(0) : END",
        "40 R() = U() . V()",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "7\n");
}

// D3: the result is calculated in full before the target changes

#[test]
fn test_the_target_can_be_a_source() {
    let output = output_of(&[
        "10 DIM A(1, 1), B(1, 1)",
        "20 A() = 1, 2, 3, 4",
        "30 B() = 1, 1, 0, 1",
        "40 A() = A() . B()",
        "50 PRINT ;A(0, 0);\" \";A(0, 1);\" \";A(1, 0);\" \";A(1, 1)",
    ]);
    assert_eq!(output, "1 3 3 7\n");
}

#[test]
fn test_a_compound_assignment_adds_the_product() {
    let output = output_of(&[
        "10 DIM U(1), V(1), R(0)",
        "20 U() = 2, 3",
        "30 V() = 4, 5",
        "40 R() = 100",
        "50 R() += U() . V()",
        "60 PRINT ;R(0)",
    ]);
    assert_eq!(output, "123\n");
}

// D4: integer and real arrays

#[test]
fn test_real_sources_and_an_integer_target_truncate_as_an_assignment() {
    let output = output_of(&[
        "10 DIM U(1), V(1), R%(0)",
        "20 U() = 1.5, 0.5",
        "30 V() = 1.5, 0.5",
        "40 R%() = U() . V()",
        "50 PRINT ;R%(0)",
    ]);
    assert_eq!(output, "2\n");
}

#[test]
fn test_integer_sources_and_a_real_target() {
    let output = output_of(&[
        "10 DIM U%(1), V%(1), R(0)",
        "20 U%() = 2, 3",
        "30 V%() = 4, 5",
        "40 R() = U%() . V%()",
        "50 PRINT ;R(0)",
    ]);
    assert_eq!(output, "23\n");
}

#[test]
fn test_integer_sources_and_an_integer_target() {
    let output = output_of(&[
        "10 DIM M%(1, 1), N%(1, 1), R%(1, 1)",
        "20 M%() = 1, 2, 3, 4",
        "30 N%() = 5, 6, 7, 8",
        "40 R%() = M%() . N%()",
        "50 PRINT ;R%(0, 0);\" \";R%(0, 1);\" \";R%(1, 0);\" \";R%(1, 1)",
    ]);
    assert_eq!(output, "19 22 43 50\n");
}

#[test]
fn test_an_integer_product_out_of_range_is_too_big() {
    let error = error_of(&[
        "10 DIM U%(1), V%(1), R%(0)",
        "20 U%() = 2000000000, 2000000000",
        "30 V%() = 1, 1",
        "40 R%() = U%() . V%()",
    ]);
    assert_eq!(error, "Too big at line 40");
}

#[test]
fn test_an_integer_and_a_real_source_is_a_type_mismatch() {
    // expra: the 2 source arrays must have the same type
    let error = error_of(&["10 DIM U(1), V%(1), R(0)", "20 R() = U() . V%()"]);
    assert_eq!(error, "Type mismatch at line 20");
}

#[test]
fn test_a_string_source_is_a_type_mismatch() {
    let error = error_of(&["10 DIM U$(1), V$(1), R(0)", "20 R() = U$() . V$()"]);
    assert_eq!(error, "Type mismatch at line 20");
}

#[test]
fn test_a_string_target_is_a_type_mismatch() {
    let error = error_of(&["10 DIM U(1), V(1), R$(0)", "20 R$() = U() . V()"]);
    assert_eq!(error, "Type mismatch at line 20");
}

// D5: the dot with other operators

#[test]
fn test_an_operator_after_the_product_is_a_syntax_error() {
    let error = error_of(&["10 DIM U(1), V(1), R(0), D(0)", "20 R() = U() . V() + D()"]);
    assert_eq!(
        error,
        "Syntax error at line 20: Unexpected + after array expression"
    );
}

#[test]
fn test_an_operator_before_the_product_is_a_syntax_error() {
    let error = error_of(&["10 DIM U(1), V(1), R(1), D(1)", "20 R() = D() + U() . V()"]);
    assert_eq!(
        error,
        "Syntax error at line 20: The dot operator must be the only operator"
    );
}

#[test]
fn test_a_minus_before_the_product_is_a_syntax_error() {
    let error = error_of(&["10 DIM U(1), V(1), R(0)", "20 R() = -U() . V()"]);
    assert_eq!(
        error,
        "Syntax error at line 20: The dot operator must be the only operator"
    );
}

#[test]
fn test_a_scalar_after_the_dot_is_a_syntax_error() {
    let error = error_of(&["10 DIM U(1), R(1)", "20 V = 2", "30 R() = U() . V"]);
    assert_eq!(error, "Syntax error at line 30: Expected array after '.'");
}

#[test]
fn test_a_list_after_the_product_is_a_syntax_error() {
    let error = error_of(&["10 DIM U(1), V(1), R(1)", "20 R() = U() . V(), 1"]);
    assert_eq!(
        error,
        "Syntax error at line 20: Unexpected , after array expression"
    );
}

// The other meanings of the full stop are unchanged

#[test]
fn test_a_short_form_and_numbers_are_unchanged() {
    let output = output_of(&["10 P.\"x\"", "20 PRINT ;1.5;\" \";.5;\" \";(2).5"]);
    assert_eq!(output, "x\n1.5 0.5 20.5\n");
}

#[test]
fn test_a_structure_member_is_unchanged() {
    let output = output_of(&["10 DIM p{x}", "20 p.x = 3", "30 PRINT ;p.x"]);
    assert_eq!(output, "3\n");
}

#[test]
fn test_a_full_stop_after_a_bracket_in_an_expression_is_a_syntax_error() {
    let error = error_of(&["10 X = (1) . 2"]);
    assert!(error.starts_with("Syntax error at line 10"), "{error}");
}

#[test]
fn test_list_shows_the_dot_operator_with_spaces() {
    let line = tokenize("10 R()=U().V()").expect("line tokenizes");
    assert_eq!(detokenize(&line).unwrap(), "10 R() = U() . V()");
}

// The real binary

#[test]
fn test_the_binary_multiplies_2_matrices() {
    let output = run_binary_read_first(
        "10 DIM M(1, 2), N(2, 1), R(1, 1)\n\
         20 M() = 1, 2, 3, 4, 5, 6\n\
         30 N() = 7, 8, 9, 10, 11, 12\n\
         40 R() = M() . N()\n\
         50 PRINT ;R(0, 0);\" \";R(0, 1);\" \";R(1, 0);\" \";R(1, 1)\n\
         RUN\n\
         EXIT\n",
    );
    assert!(output.contains("58 64 139 154"), "output: {output}");
}
