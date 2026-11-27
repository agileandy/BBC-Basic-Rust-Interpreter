//! Issue #147: EVAL calculates the value of the expression in its text
//! argument, with the variables of the program.

use crate::common::{run_binary_read_first, run_with_limit as run};

#[test]
fn test_eval_of_a_sum_is_a_number() {
    assert_eq!(
        run(&["10 PRINT EVAL(\"1+2\")"]),
        (Ok(()), "3\n".to_string())
    );
}

#[test]
fn test_eval_uses_a_variable_of_the_program() {
    let (result, output) = run(&["10 X = 7", "20 PRINT EVAL(\"X*2\")"]);
    assert_eq!((result, output), (Ok(()), "14\n".to_string()));
}

#[test]
fn test_eval_of_a_string_function_gives_a_string() {
    let (result, output) = run(&["10 A$ = \"HELLO\"", "20 PRINT EVAL(\"LEFT$(A$,1)\")"]);
    assert_eq!((result, output), (Ok(()), "H\n".to_string()));
}

#[test]
fn test_eval_with_no_brackets() {
    let (result, output) = run(&["10 A$ = \"6*7\"", "20 PRINT EVAL A$"]);
    assert_eq!((result, output), (Ok(()), "42\n".to_string()));
}

#[test]
fn test_leading_digits_above_65535_are_a_number_not_a_line_number() {
    assert_eq!(
        run(&["10 PRINT EVAL(\"70000+1\")"]),
        (Ok(()), "70001\n".to_string())
    );
}

#[test]
fn test_leading_digits_then_a_space_and_a_word_are_a_number() {
    // `1 AND 3` starts like a program line with the line number 1
    assert_eq!(
        run(&["10 PRINT EVAL(\"1 AND 3\")"]),
        (Ok(()), "1\n".to_string())
    );
}

#[test]
fn test_an_incorrect_text_gives_a_syntax_error_at_the_line_of_eval() {
    assert_eq!(
        run(&["10 X = 1", "20 PRINT EVAL(\"1+\")"]),
        (
            Err("Syntax error at line 20: Unexpected end of expression".to_string()),
            String::new()
        )
    );
}

#[test]
fn test_on_error_catches_the_error_of_the_text() {
    let (result, output) = run(&[
        "10 ON ERROR PRINT ERR; \" \"; ERL : END",
        "20 PRINT EVAL(\"1+\")",
    ]);
    assert_eq!((result, output), (Ok(()), "16 20\n".to_string()));
}

#[test]
fn test_eval_calls_a_user_function() {
    let (result, output) = run(&[
        "10 PRINT EVAL(\"FNf(2)\")",
        "20 END",
        "30 DEF FNf(N) = N*10",
    ]);
    assert_eq!((result, output), (Ok(()), "20\n".to_string()));
}

#[test]
fn test_a_nested_eval() {
    let (result, output) = run(&["10 PRINT EVAL(\"EVAL(\"\"2+3\"\")*2\")"]);
    assert_eq!((result, output), (Ok(()), "10\n".to_string()));
}

#[test]
fn test_the_target_gives_the_type_of_an_assignment() {
    let (result, output) = run(&[
        "10 A$ = EVAL(\"\"\"AB\"\"+\"\"C\"\"\")",
        "20 A = EVAL(\"1/4\")",
        "30 A% = EVAL(\"5*5\")",
        "40 PRINT A$; \" \"; A; \" \"; A%",
    ]);
    assert_eq!((result, output), (Ok(()), "ABC 0.25 25\n".to_string()));
}

#[test]
fn test_a_string_text_in_a_numeric_target_is_a_type_mismatch() {
    assert_eq!(
        run(&["10 A = EVAL(\"\"\"X\"\"\")"]),
        (Err("Type mismatch at line 10".to_string()), String::new())
    );
    assert_eq!(
        run(&["10 A$ = EVAL(\"1+1\")"]),
        (Err("Type mismatch at line 10".to_string()), String::new())
    );
}

#[test]
fn test_eval_in_a_larger_expression() {
    let (result, output) = run(&[
        "10 A$ = \"X\"",
        "20 PRINT EVAL(\"1.5\") + 1",
        "30 PRINT EVAL(\"A$\") + \"Y\"",
    ]);
    assert_eq!((result, output), (Ok(()), "2.5\nXY\n".to_string()));
}

/// A known limit: the type of an EVAL call is not known before the run, so
/// the sum of 2 string EVAL calls is a number, and gives Type mismatch.
#[test]
fn test_the_sum_of_two_string_evals_is_a_type_mismatch() {
    assert_eq!(
        run(&["10 PRINT EVAL(\"\"\"P\"\"\") + EVAL(\"\"\"Q\"\"\")"]),
        (Err("Type mismatch at line 10".to_string()), String::new())
    );
}

#[test]
fn test_the_text_is_evaluated_each_time() {
    let (result, output) = run(&[
        "10 FOR I% = 1 TO 3",
        "20 PRINT EVAL(\"I%*I%\");\" \";",
        "30 NEXT",
    ]);
    assert_eq!((result, output), (Ok(()), "1 4 9 ".to_string()));
}

#[test]
fn test_a_variable_and_an_array_with_the_name_evaluate_still_work() {
    let (result, output) = run(&[
        "10 EVALUATE = 5",
        "20 DIM EVALS(2)",
        "30 EVALS(1) = 3",
        "40 PRINT EVALUATE + EVALS(1)",
    ]);
    assert_eq!((result, output), (Ok(()), "8\n".to_string()));
}

#[test]
fn test_eval_in_the_real_binary() {
    let output = run_binary_read_first(
        "10 X = 4\n20 PRINT EVAL(\"X*X+1\")\n30 PRINT EVAL(\"STR$(X)+\"\"!\"\"\")\nRUN\nEXIT\n",
    );
    assert!(output.contains("17\n"), "output: {output:?}");
    assert!(output.contains("4!\n"), "output: {output:?}");
}
