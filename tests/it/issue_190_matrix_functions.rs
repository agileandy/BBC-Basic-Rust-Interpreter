//! Issue #190: matrix and machine learning statements and functions. These
//! are extensions of this interpreter. BBC BASIC has no form for them.

use crate::common::run_binary_read_first;
use crate::common::run_with_limit as run;

/// The 12 new names
const NAMES: [&str; 12] = [
    "MATTRANSPOSE",
    "MATINVERSE",
    "MATRELU",
    "MATSIGMOID",
    "MATTANH",
    "MATSOFTMAX",
    "MATDET",
    "MATMEAN",
    "MATSTDEV",
    "MATMIN",
    "MATMAX",
    "MATDOT",
];

/// Run the program and give the output, or panic with the error.
fn output_of(source: &[&str]) -> String {
    let (result, output) = run(source);
    assert_eq!(result, Ok(()), "output: {output}");
    output
}

/// Run the program and give the error text.
fn error_of(source: &[&str]) -> String {
    let (result, output) = run(source);
    result.expect_err(&format!("the program stops with an error: {output}"))
}

// Statements

#[test]
fn test_transpose_a_2_by_3_matrix() {
    let output = output_of(&[
        "10 DIM A(1, 2), T(2, 1)",
        "20 A() = 1, 2, 3, 4, 5, 6",
        "30 MATTRANSPOSE T(), A()",
        "40 PRINT ;T(0, 0);T(0, 1);T(1, 0);T(1, 1);T(2, 0);T(2, 1)",
    ]);
    assert_eq!(output, "142536\n");
}

#[test]
fn test_inverse_of_a_2_by_2_matrix() {
    let output = output_of(&[
        "10 DIM A(1, 1), B(1, 1)",
        "20 A() = 4, 7, 2, 6",
        "30 MATINVERSE B(), A()",
        "40 PRINT ;B(0, 0);\" \";B(0, 1);\" \";B(1, 0);\" \";B(1, 1)",
    ]);
    assert_eq!(output, "0.6 -0.7 -0.2 0.4\n");
}

#[test]
fn test_the_inverse_times_the_matrix_is_the_identity() {
    let output = output_of(&[
        "10 DIM A(2, 2), B(2, 2)",
        "20 A() = 0, 1, 2, 1, 0, 3, 4, -3, 8",
        "30 MATINVERSE B(), A()",
        "40 E = 0",
        "50 FOR R% = 0 TO 2 : FOR C% = 0 TO 2",
        "60 S = 0 : FOR K% = 0 TO 2 : S = S + A(R%, K%) * B(K%, C%) : NEXT",
        "70 IF R% = C% THEN S = S - 1",
        "80 IF ABS(S) > E THEN E = ABS(S)",
        "90 NEXT : NEXT",
        "100 IF E < 1E-9 THEN PRINT \"identity\" ELSE PRINT \"error \";E",
    ]);
    assert_eq!(output, "identity\n");
}

#[test]
fn test_a_singular_matrix_is_an_error() {
    let source = [
        "10 DIM A(1, 1), B(1, 1)",
        "20 A() = 1, 2, 2, 4",
        "30 MATINVERSE B(), A()",
    ];
    assert_eq!(error_of(&source), "Singular matrix at line 30");
}

#[test]
fn test_on_error_catches_a_singular_matrix() {
    let output = output_of(&[
        "10 ON ERROR PRINT ;ERR;\" \";REPORT$;\" \";ERL : END",
        "20 DIM A(1, 1), B(1, 1)",
        "30 A() = 1, 2, 2, 4 : B() = 9",
        "40 MATINVERSE B(), A()",
    ]);
    assert_eq!(output, "157 Singular matrix 40\n");
}

#[test]
fn test_a_singular_matrix_does_not_change_the_target() {
    let output = output_of(&[
        "10 ON ERROR PRINT ;B(0, 0);B(1, 1) : END",
        "20 DIM A(1, 1), B(1, 1)",
        "30 A() = 1, 2, 2, 4 : B() = 9",
        "40 MATINVERSE B(), A()",
    ]);
    assert_eq!(output, "99\n");
}

#[test]
fn test_inverse_of_a_matrix_that_is_not_square_is_type_mismatch() {
    let source = [
        "10 DIM A(1, 2), B(1, 2)",
        "20 A() = 1",
        "30 MATINVERSE B(), A()",
    ];
    assert_eq!(error_of(&source), "Type mismatch at line 30");
}

#[test]
fn test_a_target_of_the_wrong_size_is_type_mismatch() {
    for statement in ["MATTRANSPOSE", "MATRELU", "MATSOFTMAX"] {
        let line = format!("30 {statement} B(), A()");
        let source = ["10 DIM A(1, 2), B(2)", "20 A() = 1", &line];
        assert_eq!(error_of(&source), "Type mismatch at line 30", "{statement}");
    }
}

#[test]
fn test_transpose_of_a_vector_is_type_mismatch() {
    let source = ["10 DIM A(2), B(2)", "30 MATTRANSPOSE B(), A()"];
    assert_eq!(error_of(&source), "Type mismatch at line 30");
}

#[test]
fn test_activation_functions() {
    let output = output_of(&[
        "10 DIM A(2), B(2)",
        "20 A() = -2, 0, 3",
        "30 MATRELU B(), A() : PRINT ;B(0);\" \";B(1);\" \";B(2)",
        "40 MATSIGMOID B(), A() : PRINT ;B(1);\" \";ABS(B(2) - 1 / (1 + EXP(-3))) < 1E-9",
        "50 MATTANH B(), A() : PRINT ;B(1);\" \";ABS(B(0) + 0.96402758) < 1E-8",
    ]);
    assert_eq!(output, "0 0 3\n0.5 -1\n0 -1\n");
}

#[test]
fn test_softmax_of_a_vector_is_over_all_elements() {
    let output = output_of(&[
        "10 DIM A(3), B(3)",
        "20 A() = 1, 1, 1, 1",
        "30 MATSOFTMAX B(), A()",
        "40 PRINT ;B(0);\" \";B(3)",
    ]);
    assert_eq!(output, "0.25 0.25\n");
}

#[test]
fn test_softmax_of_a_matrix_is_over_each_row() {
    let output = output_of(&[
        "10 DIM A(1, 1), B(1, 1)",
        "20 A() = 5, 5, 0, 1000",
        "30 MATSOFTMAX B(), A()",
        "40 PRINT ;B(0, 0);\" \";B(0, 1);\" \";B(1, 0);\" \";B(1, 1)",
    ]);
    assert_eq!(output, "0.5 0.5 0 1\n");
}

#[test]
fn test_the_target_and_the_source_can_be_the_same_array() {
    let output = output_of(&[
        "10 DIM A(1, 1), V(2)",
        "20 A() = 1, 2, 3, 4 : V() = -1, 0, 1",
        "30 MATTRANSPOSE A(), A()",
        "40 MATRELU V(), V()",
        "50 PRINT ;A(0, 0);A(0, 1);A(1, 0);A(1, 1);\" \";V(0);V(1);V(2)",
    ]);
    assert_eq!(output, "1324 001\n");
}

#[test]
fn test_an_integer_target_converts_as_an_assignment_does() {
    let output = output_of(&[
        "10 DIM A(2), B%(2)",
        "20 A() = -1.5, 2.7, 3",
        "30 MATRELU B%(), A()",
        "40 PRINT ;B%(0);\" \";B%(1);\" \";B%(2)",
    ]);
    assert_eq!(output, "0 2 3\n");
}

#[test]
fn test_a_string_array_is_type_mismatch() {
    let statement = ["10 DIM A$(1), B(1)", "30 MATRELU B(), A$()"];
    assert_eq!(error_of(&statement), "Type mismatch at line 30");
    let function = ["10 DIM A$(1)", "30 PRINT ;MATMAX(A$())"];
    assert_eq!(error_of(&function), "Type mismatch at line 30");
}

#[test]
fn test_an_array_with_no_dim_is_an_array_error() {
    assert_eq!(
        error_of(&["10 DIM B(1)", "30 MATRELU B(), A()"]),
        "Array at line 30"
    );
    assert_eq!(error_of(&["30 PRINT ;MATMEAN(A())"]), "Array at line 30");
}

#[test]
fn test_a_statement_with_no_second_array_is_a_syntax_error() {
    let error = error_of(&["10 DIM B(1)", "20 MATRELU B()"]);
    assert_eq!(
        error,
        "Syntax error at line 20: MATRELU requires a target array and a source array"
    );
}

// Functions

#[test]
fn test_determinant() {
    let output = output_of(&[
        "10 DIM A(2, 2), B(1, 1)",
        "20 A() = 2, -3, 1, 2, 0, -1, 1, 4, 5",
        "30 B() = 1, 2, 2, 4",
        "40 PRINT ;MATDET(A());\" \";MATDET(B())",
    ]);
    assert_eq!(output, "49 0\n");
}

#[test]
fn test_determinant_of_a_matrix_that_is_not_square_is_type_mismatch() {
    let source = ["10 DIM A(1, 2)", "20 PRINT ;MATDET(A())"];
    assert_eq!(error_of(&source), "Type mismatch at line 20");
}

#[test]
fn test_statistics() {
    let output = output_of(&[
        "10 DIM A(1, 3)",
        "20 A() = 2, 4, 4, 4, 5, 5, 7, 9",
        "30 PRINT ;MATMEAN(A());\" \";MATSTDEV(A());\" \";MATMIN(A());\" \";MATMAX(A())",
    ]);
    assert_eq!(output, "5 2 2 9\n");
}

#[test]
fn test_a_function_reads_an_integer_array_as_numbers() {
    let output = output_of(&[
        "10 DIM A%(3)",
        "20 A%() = 1, 2, 3, 4",
        "30 PRINT ;MATMEAN(A%());\" \";MATMAX(A%())",
        "40 X% = MATMEAN(A%()) : PRINT ;X%",
    ]);
    assert_eq!(output, "2.5 4\n2\n");
}

#[test]
fn test_dot_product_of_2_vectors() {
    let output = output_of(&[
        "10 DIM A(2), B%(2)",
        "20 A() = 1, 2, 3 : B%() = 4, -5, 6",
        "30 PRINT ;MATDOT(A(), B%()) * 2",
    ]);
    assert_eq!(output, "24\n");
}

#[test]
fn test_dot_product_of_different_sizes_is_type_mismatch() {
    let source = ["10 DIM A(2), B(3)", "20 PRINT ;MATDOT(A(), B())"];
    assert_eq!(error_of(&source), "Type mismatch at line 20");
}

#[test]
fn test_a_function_with_the_wrong_argument_count_is_a_syntax_error() {
    let source = ["10 DIM A(2)", "20 PRINT ;MATDOT(A())"];
    assert_eq!(
        error_of(&source),
        "Syntax error at line 20: MATDOT requires 2 arguments"
    );
}

#[test]
fn test_a_function_argument_that_is_not_an_array_is_type_mismatch() {
    let source = ["10 X = 3", "20 PRINT ;MATMEAN(X)"];
    assert_eq!(error_of(&source), "Type mismatch at line 20");
}

// Old programs do not change

#[test]
fn test_a_variable_with_each_new_name_still_works() {
    for name in NAMES {
        let source = [
            format!("10 {name} = 3"),
            format!("20 {name} = {name} + 1"),
            format!("30 PRINT ;{name}"),
        ];
        let lines: Vec<&str> = source.iter().map(String::as_str).collect();
        assert_eq!(output_of(&lines), "4\n", "{name}");
    }
}

#[test]
fn test_an_array_with_each_new_name_still_works() {
    for name in NAMES {
        let source = [
            format!("10 DIM {name}(2)"),
            format!("20 {name}(1) = 7"),
            format!("30 {name}() += 1"),
            format!("40 PRINT ;{name}(0);\" \";{name}(1)"),
        ];
        let lines: Vec<&str> = source.iter().map(String::as_str).collect();
        assert_eq!(output_of(&lines), "1 8\n", "{name}");
    }
}

// The real binary

#[test]
fn test_the_binary_runs_a_matrix_program() {
    let output = run_binary_read_first(
        "10 DIM A(1, 1), B(1, 1)\n\
         20 A() = 4, 7, 2, 6\n\
         30 MATINVERSE B(), A()\n\
         40 PRINT ;MATDET(A());\" \";B(0, 0);\" \";B(1, 1)\n\
         RUN\n\
         EXIT\n",
    );
    assert!(output.contains("10 0.6 0.4"), "output: {output}");
}
