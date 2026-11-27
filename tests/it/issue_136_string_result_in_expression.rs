//! Issue #136: an expression with an operator finds the type of a
//! single-line user function from its body. Two functions with no `$` in
//! their names that give strings join as strings.

use crate::common::run_with_limit as run;

#[test]
fn test_print_join_of_two_string_functions() {
    let (result, output) = run(&[
        "10 PRINT FNt + FNu",
        "20 END",
        "30 DEF FNt = \"a\"",
        "40 DEF FNu = \"b\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "ab\n");
}

#[test]
fn test_assign_join_of_two_string_functions() {
    let (result, output) = run(&[
        "10 A$ = FNt + FNu : PRINT A$",
        "20 END",
        "30 DEF FNt = \"a\"",
        "40 DEF FNu = \"b\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "ab\n");
}

#[test]
fn test_if_string_function_equals_a_string() {
    let (result, output) = run(&[
        "10 IF FNt = \"a\" THEN PRINT \"yes\" ELSE PRINT \"no\"",
        "20 END",
        "30 DEF FNt = \"a\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "yes\n");
}

#[test]
fn test_if_two_string_functions_compare() {
    let (result, output) = run(&[
        "10 IF FNt < FNu THEN PRINT \"yes\" ELSE PRINT \"no\"",
        "20 END",
        "30 DEF FNt = \"a\"",
        "40 DEF FNu = \"b\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "yes\n");
}

#[test]
fn test_len_of_join_of_two_string_functions() {
    let (result, output) = run(&[
        "10 PRINT LEN(FNt + FNu)",
        "20 END",
        "30 DEF FNt = \"a\"",
        "40 DEF FNu = \"bc\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "3\n");
}

#[test]
fn test_string_parameter_gives_string_body() {
    let (result, output) = run(&[
        "10 PRINT FNw(\"a\") + FNw(\"b\")",
        "20 END",
        "30 DEF FNw(N$) = N$ + \"!\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "a!b!\n");
}

#[test]
fn test_function_that_gives_a_string_function_result_joins() {
    let (result, output) = run(&[
        "10 PRINT FNo + FNo",
        "20 END",
        "30 DEF FNo = FNi",
        "40 DEF FNi = \"in\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "inin\n");
}

#[test]
fn test_join_of_two_number_functions_does_not_change() {
    let (result, output) = run(&[
        "10 PRINT FNa + FNb; \" \"; FNa = FNb",
        "20 END",
        "30 DEF FNa = 1.5",
        "40 DEF FNb = 2",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "3.5 0\n");
}

#[test]
fn test_number_parameter_gives_number_body() {
    let (result, output) = run(&[
        "10 PRINT FNd(2) + FNd(3)",
        "20 END",
        "30 DEF FNd(N) = N * 2",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "10\n");
}

#[test]
fn test_each_function_body_runs_one_time() {
    let (result, output) = run(&[
        "10 C% = 0 : D% = 0",
        "20 PRINT FNt + FNu",
        "30 PRINT C%; \" \"; D%",
        "40 END",
        "50 DEF FNt = LEFT$(\"ab\", FNc)",
        "60 DEF FNu = LEFT$(\"cd\", FNd)",
        "70 DEF FNc",
        "80 C% = C% + 1",
        "90 = 1",
        "100 DEF FNd",
        "110 D% = D% + 1",
        "120 = 1",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "ac\n1 1\n");
}

#[test]
fn test_function_that_calls_itself_does_not_hang() {
    let (result, output) = run(&[
        "10 PRINT FNr(2) + 1",
        "20 END",
        "30 DEF FNr(N) = FNr(1 / (N - 1))",
    ]);

    assert_eq!(result, Err("Division by zero at line 30".to_string()));
    assert_eq!(output, "");
}

#[test]
fn test_two_functions_that_call_each_other_do_not_hang() {
    let (result, output) = run(&[
        "10 PRINT FNa(2) + 1",
        "20 END",
        "30 DEF FNa(N) = FNb(N)",
        "40 DEF FNb(N) = FNa(1 / (N - 1))",
    ]);

    assert_eq!(result, Err("Division by zero at line 40".to_string()));
    assert_eq!(output, "");
}

/// Known limit (D2): the type of a multi-line function result is known only
/// after its body runs, so a join of two such results is a type mismatch.
#[test]
fn test_join_of_two_multi_line_string_functions_is_a_known_limit() {
    let (result, output) = run(&[
        "10 PRINT FNt + FNu",
        "20 END",
        "30 DEF FNt",
        "40 = \"a\"",
        "50 DEF FNu",
        "60 = \"b\"",
    ]);

    assert_eq!(result, Err("Type mismatch at line 40".to_string()));
    assert_eq!(output, "");
}
