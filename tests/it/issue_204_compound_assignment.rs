//! Issue #204: compound assignment, such as `S% += 2`, for a variable, an
//! array element and a structure member. The reference is the manual of BBC
//! BASIC for Windows, page bbcwin2.html, "Compound assignment": the
//! operators are +=, -=, *=, /=, ^=, AND=, OR=, EOR=, MOD= and DIV=, and
//! only += applies to a string.

use crate::common::run_binary_read_first as run_binary;
use crate::common::run_with_limit as run;
use bbc_basic_interpreter::tokenizer::{detokenize, tokenize};

/// Run the program and give the output, or panic with the error.
fn output_of(source: &[&str]) -> String {
    let (result, output) = run(source);
    assert_eq!(result, Ok(()), "output: {output}");
    output
}

#[test]
fn test_add_to_an_integer_variable() {
    let output = output_of(&["10 S% = 1 : S% += 2", "20 PRINT ;S%"]);
    assert_eq!(output, "3\n");
}

#[test]
fn test_each_operator_for_an_integer_variable() {
    let output = output_of(&[
        "10 S% = 11",
        "20 S% += 2 : PRINT ;S%",
        "30 S% -= 3 : PRINT ;S%",
        "40 S% *= 4 : PRINT ;S%",
        "50 S% /= 8 : PRINT ;S%",
        "60 S% ^= 2 : PRINT ;S%",
        "70 S% MOD= 7 : PRINT ;S%",
        "80 S% += 10 : S% DIV= 3 : PRINT ;S%",
        "90 S% AND= 6 : PRINT ;S%",
        "100 S% OR= 3 : PRINT ;S%",
        "110 S% EOR= 5 : PRINT ;S%",
    ]);
    assert_eq!(output, "13\n10\n40\n5\n25\n4\n4\n4\n7\n2\n");
}

#[test]
fn test_each_arithmetic_operator_for_a_real_variable() {
    let output = output_of(&[
        "10 X = 1.5",
        "20 X += 1 : PRINT ;X",
        "30 X -= 0.25 : PRINT ;X",
        "40 X *= 2 : PRINT ;X",
        "50 X /= 4 : PRINT ;X",
        "60 X ^= 2 : PRINT ;X",
    ]);
    assert_eq!(output, "2.5\n2.25\n4.5\n1.125\n1.265625\n");
}

#[test]
fn test_add_to_a_string_variable() {
    let output = output_of(&["10 A$ = \"ab\" : A$ += \"cd\"", "20 PRINT A$"]);
    assert_eq!(output, "abcd\n");
}

#[test]
fn test_the_right_side_is_one_expression_in_brackets() {
    let output = output_of(&["10 S% = 1 : S% += 2 * 3", "20 PRINT ;S%"]);
    assert_eq!(output, "7\n");
}

#[test]
fn test_an_integer_target_uses_the_conversion_of_an_assignment() {
    let output = output_of(&["10 S% = 1 : S% += 2.7", "20 PRINT ;S%"]);
    assert_eq!(output, "3\n");
}

#[test]
fn test_add_to_an_array_element_evaluates_the_index_one_time() {
    let output = output_of(&[
        "10 DIM A(3), B%(2), C$(1)",
        "20 A(1) = 5 : I% = 2 : B%(I%) = 7 : C$(1) = \"x\"",
        "30 A(FNindex) += 10 : B%(I%) *= 3 : C$(1) += \"y\"",
        "40 PRINT ;A(1);\" \";N%;\" \";B%(2);\" \";C$(1)",
        "50 END",
        "60 DEF FNindex",
        "70 N% = N% + 1",
        "80 = 1",
    ]);
    assert_eq!(output, "15 1 21 xy\n");
}

#[test]
fn test_compound_assignment_to_a_structure_member() {
    let output = output_of(&[
        "10 DIM p{x, n%, s$}",
        "20 p.x = 1.5 : p.n% = 4 : p.s$ = \"a\"",
        "30 p.x *= 2 : p.n% DIV= 3 : p.s$ += \"b\"",
        "40 PRINT ;p.x;\" \";p.n%;\" \";p.s$",
    ]);
    assert_eq!(output, "3 1 ab\n");
}

#[test]
fn test_compound_assignment_to_a_member_of_an_element() {
    let output = output_of(&[
        "10 DIM t{(2) x}",
        "20 t{(1)}.x = 5",
        "30 t{(1)}.x -= 2",
        "40 PRINT ;t{(1)}.x",
    ]);
    assert_eq!(output, "3\n");
}

#[test]
fn test_compound_assignment_creates_a_variable() {
    // The manual, "Creation of variables": "Value += 1"
    let output = output_of(&[
        "10 Value += 1 : N% += 5 : B$ += \"x\"",
        "20 PRINT ;Value;\" \";N%;\" \";B$",
    ]);
    assert_eq!(output, "1 5 x\n");
}

// LIST

/// Each line lists as it is typed here
const LISTED: [&str; 13] = [
    "10 S% += 2",
    "20 X -= 1",
    "30 A(I%) *= 2",
    "40 p.x /= 2",
    "50 Y ^= 2",
    "60 S% AND= 1",
    "70 S% DIV= 2",
    "80 S% EOR= 3",
    "90 S% MOD= 4",
    "100 S% OR= 5",
    "110 A() += 1",
    "120 A$ += \"x\"",
    "130 t{(1)}.x -= 1",
];

#[test]
fn test_each_form_lists_as_typed_and_gives_the_same_tokens() {
    for source in LISTED {
        let line = tokenize(source).expect("line tokenizes");
        let listed = detokenize(&line).expect("line detokenizes");
        assert_eq!(listed, source);
        assert_eq!(tokenize(&listed).expect("listed line tokenizes"), line);
    }
}

#[test]
fn test_list_in_the_interpreter_shows_the_compound_form() {
    let output = run_binary("10 S% = 1 : S% += 2\nLIST\nEXIT\n");
    assert!(output.contains("10 S% = 1:S% += 2\n"), "output: {output}");
}

#[test]
fn test_subtract_from_a_string_is_a_type_mismatch() {
    let (result, _) = run(&["10 A$ = \"ax\" : A$ -= \"x\""]);
    assert_eq!(result, Err("Type mismatch at line 10".to_string()));
}
