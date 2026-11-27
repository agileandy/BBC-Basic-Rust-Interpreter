//! Issue 150: a statement splits its values only at a comma that is not in
//! brackets. A comma in an array element, in the arguments of a function, or
//! in a string literal is part of the value.

use crate::common::{run_lines, run_with_replies};

/// The first line of each program: a 2-dimension array with values.
const TABLE: &str =
    "10 MODE 1 : DIM P(2,2) : P(1,1) = 100 : P(1,2) = 100 : P(2,1) = 200 : P(2,2) = 0";

/// Remove the screen clear that MODE writes before the output.
fn without_clear(output: &str) -> &str {
    output.strip_prefix("\x1b[2J\x1b[H").unwrap_or(output)
}

#[test]
fn test_move_and_draw_take_an_array_element() {
    let (result, output) = run_lines(&[
        TABLE,
        "20 MOVE P(1,1), 100",
        "30 DRAW P(1,1) + 50, 100",
        "40 PRINT POINT(125,100); \" \"; POINT(125,200)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(without_clear(&output), "         3 0\n");
}

#[test]
fn test_plot_takes_array_elements() {
    let (result, output) = run_lines(&[
        TABLE,
        "20 PLOT 69, P(1,2), P(2,1)",
        "30 PRINT POINT(100,200); \" \"; POINT(200,100)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(without_clear(&output), "         3 0\n");
}

#[test]
fn test_vdu_takes_a_function_with_commas() {
    let (result, output) = run_lines(&[
        "10 A$ = \"ABC\"",
        "20 VDU ASC(MID$(A$, 2, 1)), 10",
        "30 PRINT \"x\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "B\nx\n");
}

#[test]
fn test_origin_takes_a_function_with_a_string_argument() {
    let (result, output) = run_lines(&[
        "10 MODE 1 : A$ = \"abc\"",
        "20 ORIGIN INSTR(A$, \"b\") * 100, 0",
        "30 PLOT 69, 0, 100",
        "40 ORIGIN 0, 0 : PRINT POINT(200,100); \" \"; POINT(0,100)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(without_clear(&output), "         3 0\n");
}

#[test]
fn test_gcol_takes_an_array_element() {
    // P(2,2) is 0, so the point has colour 0 and POINT gives 0
    let (result, output) = run_lines(&[
        TABLE,
        "20 GCOL 0, P(2,2)",
        "30 PLOT 69, 100, 100",
        "40 PRINT POINT(100,100)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(without_clear(&output), "         0\n");
}

#[test]
fn test_circle_ellipse_and_rectangle_take_array_elements() {
    let (result, output) = run_lines(&[
        TABLE,
        "20 CIRCLE P(2,1) * 2, P(2,1) * 2, P(1,1)",
        "30 ELLIPSE P(2,1) * 4, P(2,1) * 2, P(1,1), P(1,2) / 2",
        "40 RECTANGLE P(1,1), P(1,2), P(1,1) / 2, P(1,2) / 2",
        "50 PRINT POINT(500,400); \" \"; POINT(900,400); \" \"; POINT(100,100); \" \"; POINT(300,300)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(without_clear(&output), "         3 3 3 0\n");
}

#[test]
fn test_fill_takes_array_elements() {
    let (result, output) = run_lines(&[TABLE, "20 FILL P(1,1), P(1,2)", "30 PRINT POINT(100,100)"]);

    assert_eq!(result, Ok(()));
    assert_eq!(without_clear(&output), "         3\n");
}

#[test]
fn test_stereo_takes_array_elements() {
    let (result, output) = run_lines(&[
        "10 DIM P(2,2) : P(1,1) = 1 : P(2,2) = 0",
        "20 STEREO P(1,1), P(2,2)",
        "30 PRINT \"done\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "done\n");
}

#[test]
fn test_a_comma_in_a_string_literal_does_not_split_the_values() {
    let (result, output) = run_lines(&[
        "10 MODE 1",
        "20 PLOT 69, LEN(\"a,b\") * 100, 100",
        "30 PRINT POINT(300,100)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(without_clear(&output), "         3\n");
}

#[test]
fn test_a_list_value_takes_a_string_with_a_comma() {
    let (result, output) = run_lines(&[
        "10 L% = NEWLIST",
        "20 LISTADD L%, \"a,b\"",
        "30 PRINT LISTGET$(L%, 0)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "a,b\n");
}

#[test]
fn test_an_array_index_takes_an_array_element() {
    let (result, output) = run_lines(&[
        "10 DIM P(2,2), Q(3,3) : P(1,1) = 1 : P(2,2) = 2",
        "20 Q(P(1,1), P(2,2)) = 9",
        "30 PRINT Q(1,2)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         9\n");
}

#[test]
fn test_input_to_an_array_element_with_an_array_element_index() {
    let (result, output) = run_with_replies(
        &[
            "10 DIM P(2,2), Q(3,3) : P(1,1) = 1 : P(2,2) = 2",
            "20 INPUT Q(P(1,1), P(2,2))",
            "30 PRINT Q(1,2)",
        ],
        &["8"],
    );

    assert_eq!(result, Ok(()));
    assert!(output.ends_with("8\n"), "output was {output:?}");
}

#[test]
fn test_file_statements_take_an_array_element_as_the_handle() {
    let path =
        std::env::temp_dir().join(format!("issue_150_file_handle_{}.txt", std::process::id()));
    let path = path.to_str().expect("path is text");
    let (result, output) = run_lines(&[
        "10 DIM P(2,2), H(3,3) : P(1,1) = 1 : P(2,2) = 2",
        &format!("20 H(1,2) = OPENOUT(\"{path}\")"),
        "30 PRINT# H(P(1,1), P(2,2)), 42",
        "40 BPUT# H(P(1,1), P(2,2)), 65",
        "50 CLOSE# H(1,2)",
        &format!("60 H(1,2) = OPENIN(\"{path}\")"),
        "70 INPUT# H(P(1,1), P(2,2)), X",
        "80 PRINT X; \" \"; BGET#H(1,2)",
        "90 CLOSE# H(1,2)",
    ]);
    let _ = std::fs::remove_file(path);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "        42 65\n");
}

#[test]
fn test_unmatched_brackets_give_a_syntax_error() {
    let (result, _) = run_lines(&["10 DIM P(2,2)", "20 MOVE P(1,1, 100"]);

    assert_eq!(
        result,
        Err("Syntax error at line 20: Expected ')' in array access".to_string())
    );
}

#[test]
fn test_proc_arguments_with_commas_in_brackets_still_work() {
    let (result, output) = run_lines(&[
        "10 DIM P(2,2) : P(1,1) = 3 : A$ = \"abcd\"",
        "20 PROCa(P(1,1), MID$(A$, 1, 2))",
        "30 END",
        "40 DEF PROCa(X, Y$) : PRINT X; \" \"; Y$ : ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         3 ab\n");
}

#[test]
fn test_print_items_with_commas_in_brackets_still_work() {
    let (result, output) = run_lines(&[
        "10 DIM P(2,2) : P(1,1) = 1 : P(2,2) = 2",
        "20 PRINT P(1,1), P(2,2)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         1         2\n");
}
