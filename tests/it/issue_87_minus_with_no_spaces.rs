//! Issue 87: a subtraction with no spaces, such as 5-3 or X%-1.
//!
//! A `-` starts a negative number only when no value is before it. After a
//! value, the `-` is the subtract operator.

use crate::common::run_timed_stops as run;

/// Run the program and check that it stops with no error and gives the output.
fn check(source: &[&str], expected: &str) {
    let (result, output) = run(source);
    assert_eq!(result, Ok(()), "program: {:?}", source);
    assert_eq!(output, expected, "program: {:?}", source);
}

#[test]
fn test_number_minus_number() {
    check(&["10 PRINT 5-3"], "2\n");
}

#[test]
fn test_variable_minus_number() {
    check(&["10 X%=2 : PRINT X%-1"], "1\n");
}

#[test]
fn test_assignment_of_variable_minus_number() {
    check(&["10 X%=3", "20 Y%=X%-1", "30 PRINT Y%"], "2\n");
}

#[test]
fn test_variable_minus_variable() {
    check(&["10 A=7 : B=2", "20 PRINT A-B"], "5\n");
}

#[test]
fn test_array_element_minus_number() {
    check(&["10 DIM A(2)", "20 A(1)=9", "30 PRINT A(1)-1"], "8\n");
}

#[test]
fn test_multiply_by_negative_number() {
    check(&["10 PRINT 2*-3"], "-6\n");
}

#[test]
fn test_two_subtractions() {
    check(&["10 PRINT 10-2-3"], "5\n");
}

#[test]
fn test_function_result_minus_number() {
    check(&["10 PRINT LEN(\"ABC\")-1"], "2\n");
}

#[test]
fn test_pi_minus_number() {
    check(&["10 PRINT INT(PI-3)"], "0\n");
}

#[test]
fn test_true_minus_number() {
    check(&["10 PRINT TRUE-1"], "-2\n");
}

#[test]
fn test_variable_minus_decimal_point_number() {
    check(&["10 X=2", "20 PRINT X-.5"], "1.5\n");
}

#[test]
fn test_add_negative_number() {
    check(&["10 X%=5", "20 PRINT X%+-1"], "4\n");
}

#[test]
fn test_for_with_negative_step_runs_three_times() {
    check(
        &["10 FOR I% = 3 TO 1 STEP -1", "20 PRINT I%", "30 NEXT I%"],
        "3\n2\n1\n",
    );
}

#[test]
fn test_if_compare_with_negative_number() {
    check(
        &[
            "10 X%=-1",
            "20 IF X%=-1 THEN PRINT \"yes\" ELSE PRINT \"no\"",
        ],
        "yes\n",
    );
}

#[test]
fn test_print_negative_number_after_comma() {
    let (result, output) = run(&["10 PRINT 1,-2"]);
    assert_eq!(result, Ok(()));
    // Only the 2 values are checked here, not the field layout of the comma.
    assert_eq!(
        output.split_whitespace().collect::<Vec<_>>(),
        vec!["1", "-2"]
    );
}

#[test]
fn test_smallest_integer() {
    check(&["10 A% = -2147483648", "20 PRINT A%"], "-2147483648\n");
}

#[test]
fn test_minus_before_bracket() {
    check(&["10 PRINT -(1+2)"], "-3\n");
}

#[test]
fn test_negative_number_in_brackets() {
    check(&["10 PRINT (-3)"], "-3\n");
}

#[test]
fn test_read_negative_data() {
    check(
        &["10 READ A%, B%", "20 PRINT A%+B%", "30 DATA -1,-2"],
        "-3\n",
    );
}
