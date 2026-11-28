//! Issue #206: an array, a variable or a structure named EACH can be the
//! control variable of a normal FOR. FOR EACH is the loop over a collection
//! (issue #141) only when a name follows EACH.

use crate::common::run_with_limit as run;

#[test]
fn test_an_element_of_an_array_named_each_is_the_for_variable() {
    let (result, output) = run(&[
        "10 DIM EACH(2)",
        "20 FOR EACH(1) = 1 TO 2",
        "30 PRINT ;EACH(1);",
        "40 NEXT EACH(1)",
        "50 PRINT \" \";EACH(1);\" \";EACH(2)",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "12 3 0\n");
}

#[test]
fn test_a_variable_named_each_is_the_for_variable() {
    let (result, output) = run(&["10 FOR EACH = 1 TO 2", "20 PRINT ;EACH;", "30 NEXT EACH"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "12");
}

#[test]
fn test_a_member_of_a_structure_named_each_is_the_for_variable() {
    let (result, output) = run(&[
        "10 DIM EACH{x}",
        "20 FOR EACH.x = 1 TO 2",
        "30 PRINT ;EACH.x;",
        "40 NEXT",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "12");
}

#[test]
fn test_for_each_still_loops_over_a_collection() {
    let (result, output) = run(&[
        "10 L = NEWLIST",
        "20 LISTADD L, 3 : LISTADD L, 4",
        "30 FOR EACH V IN L",
        "40 PRINT ;V;",
        "50 NEXT",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "34");
}

#[test]
fn test_for_each_over_an_element_of_an_array_named_each_is_an_error() {
    let (result, _) = run(&["10 DIM EACH(2)", "20 FOR EACH EACH(1) IN L", "30 NEXT"]);
    assert_eq!(
        result,
        Err(
            "Syntax error at line 20: FOR EACH needs a variable name, not an array element or a member"
                .to_string()
        )
    );
}
