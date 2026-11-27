//! Issue #198: the control variable of a FOR loop can be a structure member
//! or an array element. The manual of BBC BASIC for Windows (bbcwin5.html,
//! FOR) says: "Any numeric assignable item may be used as the control
//! variable".

use crate::common::run_binary_read_first as run_binary;
use crate::common::run_with_limit as run;

#[test]
fn test_a_structure_member_is_the_for_variable() {
    let (result, output) = run(&[
        "10 DIM p{x}",
        "20 FOR p.x = 1 TO 3",
        "30 PRINT ;p.x;",
        "40 NEXT",
        "50 PRINT \" \";p.x",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "123 4\n");
}

#[test]
fn test_an_integer_member_is_an_integer_loop() {
    let (result, output) = run(&[
        "10 DIM p{n%}",
        "20 FOR p.n% = 1.7 TO 3.9",
        "30 PRINT ;p.n%;",
        "40 NEXT p.n%",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "123");
}

#[test]
fn test_an_array_element_is_the_for_variable() {
    let (result, output) = run(&[
        "10 DIM A(3)",
        "20 FOR A(2) = 1 TO 3",
        "30 PRINT ;A(2);",
        "40 NEXT A(2)",
        "50 PRINT \" \";A(2);\" \";A(1)",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "123 4 0\n");
}

#[test]
fn test_an_integer_array_element_is_an_integer_loop() {
    let (result, output) = run(&[
        "10 DIM A%(3)",
        "20 FOR A%(1) = 1.7 TO 3.9",
        "30 PRINT ;A%(1);",
        "40 NEXT",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "123");
}

#[test]
fn test_the_indices_are_evaluated_one_time_at_the_for() {
    let (result, output) = run(&[
        "10 DIM A(5)",
        "20 I% = 1",
        "30 FOR A(I%) = 1 TO 3",
        "40 I% = I% + 1",
        "50 NEXT",
        "60 PRINT ;A(1);\" \";A(2);\" \";A(3);\" \";I%",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "4 0 0 4\n");
}

#[test]
fn test_next_with_a_member_name() {
    let (result, output) = run(&[
        "10 DIM p{x, y}",
        "20 FOR p.x = 1 TO 2",
        "30 FOR p.y = 1 TO 2",
        "40 PRINT ;p.x;p.y;\" \";",
        "50 NEXT p.y, p.x",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "11 12 21 22 ");
}

#[test]
fn test_a_nested_loop_with_a_plain_variable() {
    let (result, output) = run(&[
        "10 DIM A(3), p{x}",
        "20 FOR A(1) = 1 TO 2",
        "30 FOR I = 1 TO 2",
        "40 FOR p.x = 1 TO 2",
        "50 PRINT ;A(1);I;p.x;\" \";",
        "60 NEXT p.x",
        "70 NEXT I",
        "80 NEXT A(1)",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "111 112 121 122 211 212 221 222 ");
}

#[test]
fn test_2_elements_of_one_array_are_2_loops() {
    let (result, output) = run(&[
        "10 DIM A(3)",
        "20 FOR A(1) = 1 TO 2",
        "30 FOR A(2) = 1 TO 2",
        "40 PRINT ;A(1);A(2);\" \";",
        "50 NEXT A(2)",
        "60 NEXT A(1)",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "11 12 21 22 ");
}

#[test]
fn test_next_names_the_element_with_its_index_now() {
    let (result, output) = run(&[
        "10 DIM A(3)",
        "20 J% = 2",
        "30 FOR A(J%) = 1 TO 2",
        "40 PRINT ;A(2);",
        "50 NEXT A(1+1)",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "12");
}

#[test]
fn test_a_for_of_the_same_element_closes_the_open_loop() {
    // Issue #151: a FOR of a target that has an open loop closes that loop
    // and each loop above it
    let (result, output) = run(&[
        "10 DIM A(3)",
        "20 FOR A(1) = 1 TO 2",
        "30 FOR J = 1 TO 2",
        "40 FOR A(1) = 5 TO 6",
        "50 PRINT ;A(1);",
        "60 NEXT A(1)",
        "70 NEXT J",
    ]);
    assert_eq!(result, Err("No FOR at line 70".to_string()));
    assert_eq!(output, "56");
}

#[test]
fn test_a_next_of_another_element_cannot_match_the_for() {
    let (result, _) = run(&["10 DIM A(3)", "20 FOR A(1) = 1 TO 2", "30 NEXT A(2)"]);
    assert_eq!(result, Err("Can't match FOR at line 30".to_string()));
}

#[test]
fn test_a_next_of_a_member_with_no_loop_cannot_match_the_for() {
    let (result, _) = run(&["10 DIM p{x, y}", "20 FOR p.x = 1 TO 2", "30 NEXT p.y"]);
    assert_eq!(result, Err("Can't match FOR at line 30".to_string()));
}

#[test]
fn test_a_string_target_gives_type_mismatch() {
    let cases: [&[&str]; 3] = [
        &["10 DIM p{s$}", "20 FOR p.s$ = 1 TO 3", "30 NEXT"],
        &["10 DIM A$(3)", "20 FOR A$(1) = 1 TO 3", "30 NEXT"],
        &["10 FOR A$ = 1 TO 3", "20 NEXT"],
    ];
    for source in cases {
        let (result, _) = run(source);
        let line = if source.len() == 3 { 20 } else { 10 };
        assert_eq!(
            result,
            Err(format!("Type mismatch at line {line}")),
            "{source:?}"
        );
    }
}

#[test]
fn test_a_member_that_does_not_exist_gives_no_such_variable() {
    let (result, _) = run(&["10 DIM p{x}", "20 FOR p.z = 1 TO 3", "30 NEXT"]);
    assert_eq!(result, Err("No such variable: p.z at line 20".to_string()));
}

#[test]
fn test_an_element_out_of_range_gives_subscript_out_of_range() {
    let (result, _) = run(&["10 DIM A(3)", "20 FOR A(4) = 1 TO 3", "30 NEXT"]);
    assert_eq!(result, Err("Subscript at line 20".to_string()));
}

#[test]
fn test_for_each_needs_a_plain_variable() {
    let cases: [&[&str]; 2] = [
        &["10 DIM p{x}", "20 FOR EACH p.x IN L%", "30 NEXT"],
        &["10 DIM A(3)", "20 FOR EACH A(1) IN L%", "30 NEXT"],
    ];
    for source in cases {
        let (result, _) = run(source);
        assert_eq!(
            result,
            Err(
                "Syntax error at line 20: FOR EACH needs a variable name, not an array element or a member"
                    .to_string()
            ),
            "{source:?}"
        );
    }
}

#[test]
fn test_a_plain_variable_and_an_array_of_one_name_still_work() {
    let (result, output) = run(&[
        "10 DIM A(2)",
        "20 FOR A = 1 TO 2",
        "30 A(A) = A * 10",
        "40 NEXT A",
        "50 PRINT ;A;\" \";A(1);\" \";A(2)",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "3 10 20\n");
}

#[test]
fn test_the_binary_lists_and_runs_the_loops_as_typed() {
    let output = run_binary(
        "10 DIM p{x}, A(3)\n20 FOR p.x = 1 TO 3 : FOR A(2) = 1 TO 2\n30 PRINT ;p.x;A(2);\" \";\n40 NEXT A(2), p.x\nLIST\nRUN\nEXIT\n",
    );
    // LIST writes ':' and ',' with no spaces, as for a plain FOR variable
    for line in [
        "20 FOR p.x = 1 TO 3:FOR A(2) = 1 TO 2\n",
        "40 NEXT A(2),p.x\n",
        "11 12 21 22 31 32 ",
    ] {
        assert!(output.contains(line), "{line} in output: {output}");
    }
}
