//! Issue #188: the array functions SUM, SUMLEN, MOD and DIM, as in BBC BASIC
//! for Windows. The manual pages are bbcwin2.html ("Array functions"),
//! bbcwin7.html (SUM and SUMLEN), bbcwin6.html (MOD) and bbcwin4.html ("DIM
//! as a function").

use crate::common::run_binary_read_first;
use crate::common::run_with_limit as run;

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

// SUM

#[test]
fn test_sum_of_a_real_array() {
    let output = output_of(&["10 DIM A(2)", "20 A() = 1.5, 2, 3", "30 PRINT ;SUM(A())"]);
    assert_eq!(output, "6.5\n");
}

#[test]
fn test_sum_of_an_integer_array_from_the_manual() {
    let output = output_of(&[
        "10 DIM marks%(10)",
        "20 marks%() = 0,1,2,3,4,5,6,7,8,9,10",
        "30 PRINT ;SUM(marks%())",
    ]);
    assert_eq!(output, "55\n");
}

#[test]
fn test_sum_of_a_2_dimension_real_array_from_the_manual() {
    let output = output_of(&[
        "10 DIM grid(7,7)",
        "20 grid() = PI",
        "30 PRINT ;SUM(grid())",
    ]);
    assert_eq!(output, "201.06193\n");
}

#[test]
fn test_sum_of_a_2_dimension_integer_array() {
    let output = output_of(&[
        "10 DIM A%(1, 2)",
        "20 A%() = 1, 2, 3, 4, 5, 6",
        "30 PRINT ;SUM(A%())",
    ]);
    assert_eq!(output, "21\n");
}

#[test]
fn test_sum_of_a_string_array_joins_the_elements() {
    let output = output_of(&[
        "10 DIM A$(2)",
        "20 A$() = \"ab\", \"\", \"c\"",
        "30 PRINT SUM(A$());\"|\"",
    ]);
    assert_eq!(output, "abc|\n");
}

#[test]
fn test_sum_of_a_2_dimension_string_array_is_in_storage_order() {
    let output = output_of(&[
        "10 DIM A$(1, 1)",
        "20 A$() = \"p\", \"q\", \"r\", \"s\"",
        "30 B$ = SUM(A$())",
        "40 PRINT B$",
    ]);
    assert_eq!(output, "pqrs\n");
}

#[test]
fn test_sum_of_a_string_array_above_the_limit_is_string_too_long() {
    let error = error_of(&[
        "10 DIM A$(1)",
        "20 A$() = STRING$(40000, \"x\")",
        "30 PRINT SUM(A$())",
    ]);
    assert_eq!(error, "String too long at line 30");
}

#[test]
fn test_sum_of_an_integer_array_above_the_integer_range_is_too_big() {
    let error = error_of(&[
        "10 DIM A%(1)",
        "20 A%() = 2147483647",
        "30 PRINT ;SUM(A%())",
    ]);
    assert_eq!(error, "Too big at line 30");
}

#[test]
fn test_sum_of_a_numeric_array_in_a_string_context_is_type_mismatch() {
    let error = error_of(&["10 DIM A(1)", "20 B$ = SUM(A())"]);
    assert_eq!(error, "Type mismatch at line 20");
}

#[test]
fn test_sum_of_a_string_array_in_a_numeric_context_is_type_mismatch() {
    let error = error_of(&["10 DIM A$(1)", "20 B = SUM(A$())"]);
    assert_eq!(error, "Type mismatch at line 20");
}

// SUMLEN

#[test]
fn test_sumlen_of_a_string_array() {
    let output = output_of(&[
        "10 DIM A$(2)",
        "20 A$() = \"ab\", \"\", \"cde\"",
        "30 PRINT ;SUMLEN(A$())",
    ]);
    assert_eq!(output, "5\n");
}

#[test]
fn test_sumlen_of_a_2_dimension_string_array_above_the_string_limit() {
    let output = output_of(&[
        "10 DIM A$(1, 1)",
        "20 A$() = STRING$(40000, \"x\")",
        "30 PRINT ;SUMLEN(A$())",
    ]);
    assert_eq!(output, "160000\n");
}

#[test]
fn test_sumlen_of_a_numeric_array_is_type_mismatch() {
    for array in ["A", "A%"] {
        let dim = format!("10 DIM {array}(1)");
        let print = format!("20 PRINT ;SUMLEN({array}())");
        let error = error_of(&[&dim, &print]);
        assert_eq!(error, "Type mismatch at line 20", "{array}");
    }
}

// MOD

#[test]
fn test_mod_of_a_real_array() {
    let output = output_of(&["10 DIM A(1)", "20 A() = 3, 4", "30 PRINT ;MOD(A())"]);
    assert_eq!(output, "5\n");
}

#[test]
fn test_mod_of_a_2_dimension_integer_array() {
    let output = output_of(&[
        "10 DIM A%(1, 1)",
        "20 A%() = 1, -1, 1, -1",
        "30 PRINT ;MOD(A%())",
    ]);
    assert_eq!(output, "2\n");
}

#[test]
fn test_mod_of_a_string_array_is_type_mismatch() {
    let error = error_of(&["10 DIM A$(1)", "20 PRINT ;MOD(A$())"]);
    assert_eq!(error, "Type mismatch at line 20");
}

#[test]
fn test_normalise_an_array_from_the_manual() {
    let output = output_of(&[
        "10 DIM A(1)",
        "20 A() = 3, 4",
        "30 A() /= MOD(A())",
        "40 PRINT ;A(0);\" \";A(1)",
    ]);
    assert_eq!(output, "0.6 0.8\n");
}

// DIM

#[test]
fn test_dim_of_the_cuboid_from_the_manual() {
    let output = output_of(&[
        "10 DIM cuboid(4,5,7)",
        "20 PRINT ;DIM(cuboid());\" \";DIM(cuboid(),1);\" \";DIM(cuboid(),2);\" \";DIM(cuboid(),3)",
    ]);
    assert_eq!(output, "3 4 5 7\n");
}

#[test]
fn test_dim_of_each_array_type() {
    let output = output_of(&[
        "10 DIM A%(6), B(2, 3), C$(9)",
        "20 PRINT ;DIM(A%());DIM(A%(), 1);\" \";DIM(B());DIM(B(), 1);DIM(B(), 2)",
        "30 PRINT ;DIM(C$());\" \";DIM(C$(), 1)",
    ]);
    assert_eq!(output, "16 223\n1 9\n");
}

#[test]
fn test_dim_with_a_dimension_out_of_range_is_subscript() {
    for n in ["0", "3", "-1"] {
        let print = format!("20 PRINT ;DIM(A(), {n})");
        let error = error_of(&["10 DIM A(2, 2)", &print]);
        assert_eq!(error, "Subscript at line 20", "n = {n}");
    }
}

#[test]
fn test_mean_from_the_manual() {
    let output = output_of(&[
        "10 DIM A(3)",
        "20 A() = 1, 2, 3, 6",
        "30 mean = SUM(A()) / (DIM(A(),1) + 1)",
        "40 PRINT ;mean",
    ]);
    assert_eq!(output, "3\n");
}

#[test]
fn test_array_functions_in_an_expression_and_a_condition() {
    let output = output_of(&[
        "10 DIM A%(2)",
        "20 A%() = 1, 2, 3",
        "30 X% = SUM(A%()) * 2 + DIM(A%(), 1)",
        "40 IF DIM(A%()) = 1 THEN PRINT ;X%",
    ]);
    assert_eq!(output, "14\n");
}

// The errors of D4

#[test]
fn test_an_array_with_no_dim_is_the_array_error() {
    for call in [
        "SUM(Z())",
        "SUM(Z$())",
        "SUMLEN(Z$())",
        "MOD(Z())",
        "DIM(Z())",
        "DIM(Z(), 1)",
    ] {
        let print = format!("10 PRINT ;{call}");
        let error = error_of(&[&print]);
        assert_eq!(error, "Array at line 10", "{call}");
    }
}

#[test]
fn test_a_scalar_argument_is_type_mismatch() {
    for call in ["SUM(X)", "MOD(X)", "DIM(X)"] {
        let print = format!("20 PRINT ;{call}");
        let error = error_of(&["10 X = 2", &print]);
        assert_eq!(error, "Type mismatch at line 20", "{call}");
    }
}

// D5: a local array

#[test]
fn test_a_local_array_as_the_argument() {
    let output = output_of(&[
        "10 PROCtotal",
        "20 END",
        "30 DEF PROCtotal",
        "40 LOCAL L()",
        "50 DIM L(2)",
        "60 L() = 1, 2, 4",
        "70 PRINT ;SUM(L());\" \";DIM(L(), 1);\" \";DIM(L())",
        "80 ENDPROC",
    ]);
    assert_eq!(output, "7 2 1\n");
}

// The old meanings do not change (D2 and rule 15)

#[test]
fn test_the_mod_operator_does_not_change() {
    let output = output_of(&[
        "10 A = 7 : B = 3",
        "20 PRINT ;7 MOD 3;\" \";7 MOD (A);\" \";A MOD(B)",
        "30 X = A MOD(B)",
        "40 PRINT ;X",
    ]);
    assert_eq!(output, "1 0 1\n1\n");
}

#[test]
fn test_the_dim_statement_does_not_change() {
    let output = output_of(&["10 DIM A(3)", "20 A(3) = 9", "30 PRINT ;A(3)"]);
    assert_eq!(output, "9\n");
}

#[test]
fn test_variables_and_arrays_named_sumx_and_sumlen_still_work() {
    let output = output_of(&[
        "10 SUMX = 4 : SUMLEN = 5",
        "20 DIM SUMLEN(2), SUMX%(1)",
        "30 SUMLEN(1) = 7 : SUMX%(1) = 8",
        "40 PRINT ;SUMX;\" \";SUMLEN;\" \";SUMLEN(1);\" \";SUMX%(1)",
    ]);
    assert_eq!(output, "4 5 7 8\n");
}

// The real binary

#[test]
fn test_array_functions_in_the_real_binary() {
    let output = run_binary_read_first(
        "10 DIM A%(1, 2), B$(1)\n\
         20 A%() = 1, 2, 3, 4, 5, 6\n\
         30 B$() = \"ab\", \"cd\"\n\
         40 PRINT ;SUM(A%());\" \";DIM(A%());DIM(A%(), 2);\" \";SUM(B$());SUMLEN(B$())\n\
         RUN\n\
         EXIT\n",
    );
    assert!(output.contains("21 22 abcd4"), "output: {output}");
}
