//! Issue #187: whole-array assignment and arithmetic, as in BBC BASIC for
//! Windows (manual page bbcwin2.html, "Array arithmetic" and "Initialising
//! arrays").

use crate::common::run_binary_read_first;
use crate::common::run_with_limit as run;

/// Run the program and give the output, or panic with the error.
fn output_of(source: &[&str]) -> String {
    let (result, output) = run(source);
    assert_eq!(result, Ok(()), "output: {output}");
    output
}

// Copy

#[test]
fn test_copy_a_real_array() {
    let output = output_of(&[
        "10 DIM A(2), B(2)",
        "20 B(0) = 1.5 : B(1) = 2 : B(2) = -3",
        "30 A() = B()",
        "40 PRINT ;A(0);\" \";A(1);\" \";A(2)",
    ]);
    assert_eq!(output, "1.5 2 -3\n");
}

#[test]
fn test_copy_a_2_dimension_integer_array() {
    let output = output_of(&[
        "10 DIM A%(1, 2), B%(1, 2)",
        "20 B%(0, 0) = 1 : B%(0, 2) = 3 : B%(1, 1) = 5",
        "30 A%() = B%()",
        "40 PRINT ;A%(0, 0);A%(0, 1);A%(0, 2);A%(1, 0);A%(1, 1);A%(1, 2)",
    ]);
    assert_eq!(output, "103050\n");
}

#[test]
fn test_copy_a_string_array() {
    let output = output_of(&[
        "10 DIM A$(1), B$(1, 1), C$(1, 1)",
        "20 A$(0) = \"x\" : A$(1) = \"y\"",
        "30 C$(0, 1) = \"p\" : C$(1, 0) = \"q\"",
        "40 B$() = C$()",
        "50 PRINT A$(0);A$(1);\" \";B$(0, 0);B$(0, 1);B$(1, 0);B$(1, 1)",
    ]);
    assert_eq!(output, "xy pq\n");
}

#[test]
fn test_copy_converts_a_real_to_an_integer() {
    let output = output_of(&[
        "10 DIM A%(1), B(1)",
        "20 B(0) = 2.7 : B(1) = -2.7",
        "30 A%() = B()",
        "40 PRINT ;A%(0);\" \";A%(1)",
    ]);
    assert_eq!(output, "2 -2\n");
}

// Fill

#[test]
fn test_fill_a_real_array_and_an_integer_array() {
    let output = output_of(&[
        "10 DIM A(2), M%(1, 1)",
        "20 A() = PI / 2 : M%() = 7",
        "30 PRINT ;A(0) = PI / 2;\" \";A(2) = PI / 2;\" \";M%(0, 0);M%(1, 1)",
    ]);
    assert_eq!(output, "-1 -1 77\n");
}

#[test]
fn test_fill_a_string_array() {
    let output = output_of(&[
        "10 DIM A$(2), M$(1, 1)",
        "20 A$() = \"ab\" : M$() = \"c\" + \"d\"",
        "30 PRINT A$(0);A$(2);M$(0, 0);M$(1, 1)",
    ]);
    assert_eq!(output, "ababcdcd\n");
}

#[test]
fn test_fill_an_integer_array_truncates_the_value() {
    let output = output_of(&["10 DIM A%(1)", "20 A%() = 9.9", "30 PRINT ;A%(0);A%(1)"]);
    assert_eq!(output, "99\n");
}

#[test]
fn test_fill_an_integer_array_out_of_range_is_too_big() {
    let (result, _) = run(&["10 DIM A%(1)", "20 A%() = 3E9"]);
    assert_eq!(result, Err("Too big at line 20".to_string()));
}

#[test]
fn test_let_fills_an_array() {
    let output = output_of(&["10 DIM A(1)", "20 LET A() = 4", "30 PRINT ;A(0);A(1)"]);
    assert_eq!(output, "44\n");
}

// A list of values

#[test]
fn test_a_list_sets_the_first_elements_and_the_rest_stay() {
    let output = output_of(&[
        "10 DIM A%(4)",
        "20 A%() = 9",
        "30 A%() = 1, 2, 3",
        "40 PRINT ;A%(0);A%(1);A%(2);A%(3);A%(4)",
    ]);
    assert_eq!(output, "12399\n");
}

#[test]
fn test_a_list_fills_a_2_dimension_array_row_by_row() {
    let output = output_of(&[
        "10 DIM M%(1, 2)",
        "20 M%() = 1, 2, 3, 4",
        "30 PRINT ;M%(0, 0);M%(0, 1);M%(0, 2);M%(1, 0);M%(1, 1);M%(1, 2)",
    ]);
    assert_eq!(output, "123400\n");
}

#[test]
fn test_a_list_of_strings() {
    let output = output_of(&[
        "10 DIM B$(3)",
        "20 B$() = \"Alpha\", \"Beta\", \"Gam\" + \"ma\"",
        "30 PRINT B$(0);\" \";B$(1);\" \";B$(2);\" [\";B$(3);\"]\"",
    ]);
    assert_eq!(output, "Alpha Beta Gamma []\n");
}

#[test]
fn test_a_list_with_a_value_for_each_element() {
    let output = output_of(&[
        "10 DIM A(2)",
        "20 A() = 1.5, 2 * 3, -1",
        "30 PRINT ;A(0);\" \";A(1);\" \";A(2)",
    ]);
    assert_eq!(output, "1.5 6 -1\n");
}

#[test]
fn test_a_list_with_too_many_values_is_an_error_and_changes_nothing() {
    let output = output_of(&[
        "10 ON ERROR GOTO 100",
        "20 DIM A%(1)",
        "30 A%() = 1, 2, 3",
        "40 END",
        "100 PRINT REPORT$;\" \";ERL;\" \";A%(0);A%(1)",
    ]);
    assert_eq!(
        output,
        "Syntax error: Too many values for the array 30 00\n"
    );
}

// Arithmetic with 2 arrays

#[test]
fn test_add_subtract_multiply_and_divide_2_real_arrays() {
    let output = output_of(&[
        "10 DIM A(1), B(1), C(1)",
        "20 B() = 6, 1.5 : C() = 2, 0.5",
        "30 A() = B() + C() : PRINT ;A(0);\" \";A(1)",
        "40 A() = B() - C() : PRINT ;A(0);\" \";A(1)",
        "50 A() = B() * C() : PRINT ;A(0);\" \";A(1)",
        "60 A() = B() / C() : PRINT ;A(0);\" \";A(1)",
    ]);
    assert_eq!(output, "8 2\n4 1\n12 0.75\n3 3\n");
}

#[test]
fn test_arithmetic_with_2_dimension_integer_arrays() {
    let output = output_of(&[
        "10 DIM A%(1, 1), B%(1, 1), C%(1, 1)",
        "20 B%() = 10, 20, 30, 40 : C%() = 3, 4, 5, 6",
        "30 A%() = B%() + C%() : PRINT ;A%(0, 0);\" \";A%(1, 1)",
        "40 A%() = B%() - C%() : PRINT ;A%(0, 0);\" \";A%(1, 1)",
        "50 A%() = B%() * C%() : PRINT ;A%(0, 0);\" \";A%(1, 1)",
        "60 A%() = B%() / C%() : PRINT ;A%(0, 0);\" \";A%(0, 1);\" \";A%(1, 1)",
    ]);
    assert_eq!(output, "13 46\n7 34\n30 240\n3 5 6\n");
}

#[test]
fn test_join_2_string_arrays() {
    let output = output_of(&[
        "10 DIM A$(1, 1), B$(1, 1), C$(1, 1)",
        "20 B$() = \"a\", \"b\", \"c\", \"d\" : C$() = \"1\", \"2\", \"3\", \"4\"",
        "30 A$() = B$() + C$()",
        "40 PRINT A$(0, 0);A$(0, 1);A$(1, 0);A$(1, 1)",
    ]);
    assert_eq!(output, "a1b2c3d4\n");
}

#[test]
fn test_a_string_array_with_minus_is_a_type_mismatch() {
    let (result, _) = run(&["10 DIM A$(1), B$(1)", "20 A$() = A$() - B$()"]);
    assert_eq!(result, Err("Type mismatch at line 20".to_string()));
}

// Arithmetic with a number

#[test]
fn test_a_number_on_each_side_of_an_array() {
    let output = output_of(&[
        "10 DIM A(1), B(1), I%(1)",
        "20 B() = 1, 2.5",
        "30 A() = B() * 2 : PRINT ;A(0);\" \";A(1)",
        "40 A() = 2 - B() : PRINT ;A(0);\" \";A(1)",
        "50 I%() = 10 / B() : PRINT ;I%(0);\" \";I%(1)",
        "60 A() = B() - PI : PRINT ;A(0) = 1 - PI",
    ]);
    assert_eq!(output, "2 5\n1 -0.5\n10 4\n-1\n");
}

#[test]
fn test_a_string_on_each_side_of_a_string_array() {
    let output = output_of(&[
        "10 DIM A$(1), B$(1)",
        "20 B$() = \"x\", \"y\"",
        "30 A$() = \"<\" + B$() + \">\"",
        "40 PRINT A$(0);A$(1)",
    ]);
    assert_eq!(output, "<x><y>\n");
}

#[test]
fn test_an_array_expression_is_strictly_left_to_right() {
    // The manual: "Array expressions are evaluated strictly left-to-right"
    let output = output_of(&[
        "10 DIM A(1), B(1), C(1)",
        "20 B() = 1, 2 : C() = 10, 20",
        "30 A() = B() + C() * 2 : PRINT ;A(0);\" \";A(1)",
        "40 A() = B() * 2 / C() + 3 - A() : PRINT ;A(0);\" \";A(1)",
    ]);
    assert_eq!(output, "22 44\n-18.8 -40.8\n");
}

#[test]
fn test_a_number_after_plus_binds_a_multiplication() {
    let output = output_of(&[
        "10 DIM A(1), B(1)",
        "20 B() = 1, 2",
        "30 A() = B() + 2 * 3",
        "40 PRINT ;A(0);\" \";A(1)",
    ]);
    assert_eq!(output, "7 8\n");
}

#[test]
fn test_div_mod_and_power_of_arrays() {
    let output = output_of(&[
        "10 DIM A%(1), B%(1), C%(1), R(1)",
        "20 B%() = 17, -17 : C%() = 5, 5",
        "30 A%() = B%() DIV C%() : PRINT ;A%(0);\" \";A%(1)",
        "40 A%() = B%() MOD C%() : PRINT ;A%(0);\" \";A%(1)",
        "50 R() = C%() ^ 2 : PRINT ;R(0);\" \";R(1)",
    ]);
    assert_eq!(output, "3 -3\n2 -2\n25 25\n");
}

#[test]
fn test_and_or_and_eor_of_arrays() {
    let output = output_of(&[
        "10 DIM A%(0), B%(0)",
        "20 B%() = 12",
        "30 A%() = B%() AND 10 : PRINT ;A%(0)",
        "40 A%() = B%() OR 3 : PRINT ;A%(0)",
        "50 A%() = B%() EOR 5 : PRINT ;A%(0)",
    ]);
    assert_eq!(output, "8\n15\n9\n");
}

#[test]
fn test_unary_minus_of_an_array() {
    let output = output_of(&[
        "10 DIM A(1), B(1)",
        "20 B() = 3, -4",
        "30 A() = -B()",
        "40 PRINT ;A(0);\" \";A(1)",
    ]);
    assert_eq!(output, "-3 4\n");
}

// The target in the expression

#[test]
fn test_an_array_times_2_into_itself() {
    let output = output_of(&[
        "10 DIM A%(2)",
        "20 A%() = 1, 2, 3",
        "30 A%() = A%() * 2",
        "40 PRINT ;A%(0);A%(1);A%(2)",
    ]);
    assert_eq!(output, "246\n");
}

#[test]
fn test_an_array_plus_an_other_array_into_itself() {
    let output = output_of(&[
        "10 DIM A(1), B(1)",
        "20 A() = 1, 2 : B() = 10, 20",
        "30 A() = A() + B()",
        "40 PRINT ;A(0);\" \";A(1)",
    ]);
    assert_eq!(output, "11 22\n");
}

// Compound assignment

#[test]
fn test_compound_assignment_of_arrays() {
    let output = output_of(&[
        "10 DIM A(1), B(1), S$(1)",
        "20 A() = 1, 2 : B() = 10, 20 : S$() = \"a\", \"b\"",
        "30 A() += 1 : PRINT ;A(0);\" \";A(1)",
        "40 A() -= B() : PRINT ;A(0);\" \";A(1)",
        "50 A() *= 2 : PRINT ;A(0);\" \";A(1)",
        "60 A() /= 4 : PRINT ;A(0);\" \";A(1)",
        "70 S$() += \"!\" : PRINT S$(0);S$(1)",
    ]);
    assert_eq!(output, "2 3\n-8 -17\n-16 -34\n-4 -8.5\na!b!\n");
}

#[test]
fn test_compound_assignment_of_an_integer_array_uses_an_integer_value() {
    // The manual: "A%() *= 2.5 will multiply all the elements of array A%() by 2"
    let output = output_of(&[
        "10 DIM A%(1)",
        "20 A%() = 3, 5",
        "30 A%() *= 2.5",
        "40 PRINT ;A%(0);\" \";A%(1)",
    ]);
    assert_eq!(output, "6 10\n");
}

#[test]
fn test_compound_assignment_with_a_keyword_operator() {
    let output = output_of(&[
        "10 DIM A%(1)",
        "20 A%() = 11, 22",
        "30 A%() MOD= 4 : PRINT ;A%(0);\" \";A%(1)",
        "40 A%() DIV= 2 : PRINT ;A%(0);\" \";A%(1)",
    ]);
    assert_eq!(output, "3 2\n1 1\n");
}

// Errors

#[test]
fn test_arrays_with_different_element_counts_are_a_type_mismatch() {
    for statement in [
        "30 A() = B()",
        "30 A() = B() + C()",
        "30 A() = C() * 2 + B()",
    ] {
        let (result, _) = run(&["10 DIM A(2), B(3), C(2)", "20 C() = 1", statement]);
        assert_eq!(
            result,
            Err("Type mismatch at line 30".to_string()),
            "{statement}"
        );
    }
}

#[test]
fn test_arrays_with_the_same_element_count_and_other_dimensions_agree() {
    // The manual: "the number of elements in all the arrays within an
    // expression must be the same"
    let output = output_of(&[
        "10 DIM A(5), B(1, 2)",
        "20 B() = 1, 2, 3, 4, 5, 6",
        "30 A() = B()",
        "40 PRINT ;A(0);A(2);A(3);A(5)",
    ]);
    assert_eq!(output, "1346\n");
}

#[test]
fn test_a_numeric_array_and_a_string_array_are_a_type_mismatch() {
    for statement in [
        "30 A() = B$()",
        "30 B$() = A()",
        "30 A() = \"x\"",
        "30 B$() = 5",
        "30 A() = A() + \"x\"",
    ] {
        let (result, _) = run(&["10 DIM A(1), B$(1)", "20 A() = 1", statement]);
        assert_eq!(
            result,
            Err("Type mismatch at line 30".to_string()),
            "{statement}"
        );
    }
}

#[test]
fn test_an_error_in_the_middle_changes_nothing() {
    let output = output_of(&[
        "10 ON ERROR GOTO 100",
        "20 DIM A(2), B(2), C(2)",
        "30 A() = 7 : B() = 1 : C() = 1, 0, 1",
        "40 A() = B() / C()",
        "50 END",
        "100 PRINT REPORT$;\" \";ERL;\" \";A(0);A(1);A(2)",
    ]);
    assert_eq!(output, "Division by zero 40 777\n");
}

#[test]
fn test_an_error_in_the_middle_of_a_compound_assignment_changes_nothing() {
    let output = output_of(&[
        "10 ON ERROR GOTO 100",
        "20 DIM A(2), C(2)",
        "30 A() = 7 : C() = 1, 0, 1",
        "40 A() /= C()",
        "50 END",
        "100 PRINT REPORT$;\" \";A(0);A(1);A(2)",
    ]);
    assert_eq!(output, "Division by zero 777\n");
}

#[test]
fn test_a_real_result_out_of_range_is_too_big() {
    let (result, _) = run(&["10 DIM A(1), B(1)", "20 B() = 1E300", "30 A() = B() * B()"]);
    assert_eq!(result, Err("Too big at line 30".to_string()));
}

#[test]
fn test_an_integer_result_out_of_range_is_too_big() {
    let (result, _) = run(&[
        "10 DIM A%(1), B%(1)",
        "20 B%() = 2000000000",
        "30 A%() = B%() + B%()",
    ]);
    assert_eq!(result, Err("Too big at line 30".to_string()));
}

#[test]
fn test_a_string_result_that_is_too_long() {
    let (result, _) = run(&[
        "10 DIM A$(1)",
        "20 A$() = STRING$(40000, \"x\")",
        "30 A$() = A$() + A$()",
    ]);
    assert_eq!(result, Err("String too long at line 30".to_string()));
}

#[test]
fn test_an_array_with_no_dim_is_the_array_error() {
    for (source, line) in [
        (["10 DIM A(1)", "20 A() = 1", "30 A() = B()"], 30),
        (["10 DIM B(1)", "20 B() = 1", "30 A() = B()"], 30),
        (["10 DIM B(1)", "20 B() = 1", "30 A() = 2"], 30),
    ] {
        let (result, _) = run(&source);
        assert_eq!(result, Err(format!("Array at line {line}")), "{source:?}");
    }
}

#[test]
fn test_a_word_after_an_array_expression_is_a_syntax_error() {
    let (result, _) = run(&["10 DIM A(1), B(1)", "20 A() = B() B()"]);
    assert!(
        matches!(&result, Err(text) if text.starts_with("Syntax error at line 20")),
        "{result:?}"
    );
}

// Local arrays

#[test]
fn test_a_local_array_as_a_target_and_as_an_operand() {
    let output = output_of(&[
        "10 DIM G(1)",
        "20 G() = 1, 2",
        "30 PROCp",
        "40 PRINT ;G(0);\" \";G(1)",
        "50 END",
        "60 DEF PROCp",
        "70 LOCAL L()",
        "80 DIM L(1)",
        "90 L() = G() * 10",
        "100 G() = L() + G()",
        "110 PRINT ;L(0);\" \";L(1)",
        "120 ENDPROC",
    ]);
    assert_eq!(output, "10 20\n11 22\n");
}

// Old programs

#[test]
fn test_an_old_program_with_elements_is_unchanged() {
    let output = output_of(&[
        "10 DIM A(3)",
        "20 A(1) = 2",
        "30 A(3) = A(1) * 2",
        "40 PRINT ;A(0);A(1);A(2);A(3)",
    ]);
    assert_eq!(output, "0204\n");
}

#[test]
fn test_a_variable_and_an_array_with_the_same_name() {
    let output = output_of(&[
        "10 A = 7 : A% = 8 : A$ = \"s\"",
        "20 DIM A(1), A%(1), A$(1)",
        "30 A() = A : A%() = A% : A$() = A$",
        "40 PRINT ;A;A(1);A%;A%(1);A$;A$(1)",
    ]);
    assert_eq!(output, "7788ss\n");
}

// The real binary

#[test]
fn test_the_binary_runs_an_array_program() {
    let output = run_binary_read_first(
        "10 DIM A%(2), B%(2)\n\
         20 A%() = 1, 2, 3\n\
         30 B%() = A%() * 10 + A%()\n\
         40 PRINT ;B%(0);\" \";B%(1);\" \";B%(2)\n\
         RUN\n\
         EXIT\n",
    );
    assert!(output.contains("11 22 33"), "output: {output}");
}
