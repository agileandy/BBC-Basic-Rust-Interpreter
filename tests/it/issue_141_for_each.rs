//! Issue 141: `FOR EACH v IN l` runs a loop one time for each element of a
//! list, or one time for each key of a map. This is a modern extension, not
//! BBC BASIC. EACH and IN stay valid names of variables and arrays.

use crate::common::{run_binary_read_first, run_with_limit as run};

#[test]
fn test_for_each_visits_each_number_of_a_list() {
    let (result, output) = run(&[
        "10 L = NEWLIST",
        "20 LISTADD L, 3 : LISTADD L, 1.5 : LISTADD L, -2",
        "30 FOR EACH N IN L",
        "40 PRINT N",
        "50 NEXT",
        "60 PRINT \"DONE\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "3\n1.5\n-2\nDONE\n");
}

#[test]
fn test_for_each_visits_each_string_of_a_list_and_next_each_ends_it() {
    let (result, output) = run(&[
        "10 L% = NEWLIST",
        "20 LISTADD L%, \"RED\" : LISTADD L%, \"GREEN\"",
        "30 FOR EACH C$ IN L%",
        "40 PRINT C$",
        "50 NEXT EACH",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "RED\nGREEN\n");
}

#[test]
fn test_for_each_with_an_integer_variable_and_next_with_the_name() {
    let (result, output) = run(&[
        "10 L = NEWLIST",
        "20 LISTADD L, 7 : LISTADD L, 8",
        "30 FOR EACH N% IN L : PRINT N% : NEXT N%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "7\n8\n");
}

#[test]
fn test_for_each_of_an_empty_list_runs_the_body_0_times() {
    let (result, output) = run(&[
        "10 L = NEWLIST",
        "20 FOR EACH N IN L",
        "30 PRINT \"BODY\"",
        "40 NEXT",
        "50 PRINT \"AFTER\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "AFTER\n");
}

#[test]
fn test_for_each_of_an_empty_list_skips_nested_loops_and_next_with_2_names() {
    let (result, output) = run(&[
        "10 E = NEWLIST : F = NEWLIST : LISTADD F, 1",
        "20 FOR EACH N IN E",
        "30 FOR EACH M IN F : PRINT \"INNER\" : NEXT",
        "40 FOR I = 1 TO 2 : FOR J = 1 TO 2",
        "50 PRINT \"COUNT\"",
        "60 NEXT J, I",
        "70 NEXT EACH",
        "80 PRINT \"AFTER\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "AFTER\n");
}

#[test]
fn test_for_each_of_an_empty_list_with_a_next_that_also_closes_an_outer_for() {
    let (result, output) = run(&[
        "10 E = NEWLIST",
        "20 FOR I = 1 TO 2",
        "30 FOR EACH N IN E",
        "40 PRINT \"BODY\"",
        "50 NEXT N, I",
        "60 PRINT \"AFTER \"; I",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "AFTER 3\n");
}

#[test]
fn test_for_each_of_an_empty_list_with_one_next_for_a_skipped_loop_and_the_for_each() {
    let (result, output) = run(&[
        "10 E = NEWLIST",
        "20 FOR EACH N IN E",
        "30 FOR J = 1 TO 2",
        "40 PRINT \"BODY\"",
        "50 NEXT J, N",
        "60 PRINT \"AFTER\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "AFTER\n");
}

#[test]
fn test_next_each_closes_a_for_each_in_a_for_of_a_variable_named_each() {
    let (result, output) = run(&[
        "10 L = NEWLIST : LISTADD L, \"A\"",
        "20 FOR EACH = 1 TO 2",
        "30 FOR EACH S$ IN L : PRINT EACH; S$ : NEXT EACH",
        "40 NEXT EACH",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1A\n2A\n");
}

#[test]
fn test_for_each_of_an_empty_list_with_no_next_is_an_error() {
    let (result, output) = run(&["10 L = NEWLIST", "20 FOR EACH N IN L", "30 PRINT \"BODY\""]);

    assert_eq!(
        result,
        Err("FOR EACH without matching NEXT at line 20".to_string())
    );
    assert_eq!(output, "");
}

#[test]
fn test_nested_for_each_loops() {
    let (result, output) = run(&[
        "10 A = NEWLIST : LISTADD A, 1 : LISTADD A, 2",
        "20 B = NEWLIST : LISTADD B, \"X\" : LISTADD B, \"Y\"",
        "30 FOR EACH N IN A",
        "40 FOR EACH S$ IN B",
        "50 PRINT N; S$",
        "60 NEXT S$",
        "70 NEXT N",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1X\n1Y\n2X\n2Y\n");
}

#[test]
fn test_for_each_in_a_for_loop() {
    let (result, output) = run(&[
        "10 L = NEWLIST : LISTADD L, 10 : LISTADD L, 20",
        "20 FOR I% = 1 TO 2",
        "30 FOR EACH N IN L : PRINT I% * N : NEXT",
        "40 NEXT I%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "10\n20\n20\n40\n");
}

#[test]
fn test_for_each_in_a_procedure_and_endproc_closes_it() {
    let (result, output) = run(&[
        "10 L = NEWLIST : LISTADD L, 1 : LISTADD L, 2 : LISTADD L, 3",
        "20 PROCFIRST(L)",
        "30 PROCFIRST(L)",
        "40 FOR I = 1 TO 2 : PRINT \"I\"; I : NEXT",
        "50 END",
        "60 DEF PROCFIRST(H)",
        "70 FOR EACH N IN H",
        "80 IF N = 2 THEN ENDPROC",
        "90 PRINT N",
        "100 NEXT",
        "110 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n1\nI1\nI2\n");
}

#[test]
fn test_a_body_that_adds_an_element_visits_it() {
    let (result, output) = run(&[
        "10 L = NEWLIST : LISTADD L, 1",
        "20 FOR EACH N IN L",
        "30 PRINT N",
        "40 IF N < 3 THEN LISTADD L, N + 1",
        "50 NEXT",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n2\n3\n");
}

#[test]
fn test_a_body_that_removes_elements_does_not_read_past_the_end() {
    let (result, output) = run(&[
        "10 L = NEWLIST : LISTADD L, 1 : LISTADD L, 2 : LISTADD L, 3 : LISTADD L, 4",
        "20 FOR EACH N IN L",
        "30 PRINT N",
        "40 LISTREMOVE L, LISTLEN(L) - 1",
        "50 NEXT",
        "60 PRINT \"LEFT \"; LISTLEN(L)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n2\nLEFT 2\n");
}

#[test]
fn test_a_string_variable_with_a_number_element_is_type_mismatch_at_the_for_line() {
    let (result, output) = run(&[
        "10 L = NEWLIST : LISTADD L, 1",
        "20 FOR EACH S$ IN L",
        "30 NEXT",
    ]);

    assert_eq!(result, Err("Type mismatch at line 20".to_string()));
    assert_eq!(output, "");
}

#[test]
fn test_a_numeric_variable_with_a_string_element_is_type_mismatch_at_the_next_line() {
    let (result, output) = run(&[
        "10 L = NEWLIST : LISTADD L, 1 : LISTADD L, \"TWO\"",
        "20 FOR EACH N IN L",
        "30 PRINT N",
        "40 NEXT",
    ]);

    assert_eq!(result, Err("Type mismatch at line 40".to_string()));
    assert_eq!(output, "1\n");
}

#[test]
fn test_for_each_of_a_map_visits_each_key_in_order() {
    let (result, output) = run(&[
        "10 M = NEWMAP",
        "20 MAPPUT M, \"NAME\", \"ANN\" : MAPPUT M, \"AGE\", 42",
        "30 FOR EACH K$ IN M",
        "40 PRINT K$",
        "50 NEXT",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "NAME\nAGE\n");
}

#[test]
fn test_for_each_of_a_map_with_a_numeric_variable_is_type_mismatch() {
    let (result, _) = run(&["10 M = NEWMAP", "20 FOR EACH K IN M", "30 NEXT"]);

    assert_eq!(result, Err("Type mismatch at line 20".to_string()));
}

#[test]
fn test_for_each_of_a_number_that_is_not_a_collection_is_bad_collection() {
    let (result, _) = run(&["10 FOR EACH N IN 99", "20 NEXT"]);

    assert_eq!(result, Err("Bad collection at line 10".to_string()));
}

#[test]
fn test_for_each_with_no_in_is_a_syntax_error() {
    let (result, _) = run(&["10 L = NEWLIST", "20 FOR EACH N L", "30 NEXT"]);

    assert_eq!(
        result,
        Err("Syntax error at line 20: Expected IN in FOR EACH statement".to_string())
    );
}

#[test]
fn test_a_trapped_error_in_a_for_each_body_goes_to_the_handler() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 L = NEWLIST : LISTADD L, 1 : LISTADD L, 0",
        "30 FOR EACH N IN L",
        "40 PRINT 1 / N",
        "50 NEXT",
        "60 END",
        "100 PRINT REPORT$; \" AT \"; ERL",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\nDivision by zero AT 40\n");
}

#[test]
fn test_goto_out_of_a_for_each_loop_and_a_new_loop_after_it() {
    let (result, output) = run(&[
        "10 L = NEWLIST : LISTADD L, 1 : LISTADD L, 2",
        "20 FOR EACH N IN L",
        "30 IF N = 2 THEN GOTO 60",
        "40 PRINT N",
        "50 NEXT",
        "60 FOR EACH M IN L : PRINT \"M\"; M : NEXT",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\nM1\nM2\n");
}

#[test]
fn test_variables_and_arrays_named_each_and_in_still_work() {
    let (result, output) = run(&[
        "10 FOR EACH = 1 TO 3 : PRINT EACH; : NEXT EACH",
        "20 PRINT",
        "30 IN = 5 : EACH$ = \"E\" : PRINT IN; EACH$",
        "40 DIM EACH(2), IN(2)",
        "50 EACH(1) = 6 : IN(2) = 7 : PRINT EACH(1) + IN(2)",
        "60 FOR IN = 1 TO 2 : PRINT IN; : NEXT IN",
        "70 PRINT",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "123\n5E\n13\n12\n");
}

#[test]
fn test_binary_runs_for_each_and_lists_the_line_as_typed() {
    let output = run_binary_read_first(
        "10 L = NEWLIST : LISTADD L, \"A\" : LISTADD L, \"B\"\n\
         20 FOR EACH S$ IN L : PRINT S$ : NEXT EACH\n\
         LIST 20\nRUN\nEXIT\n",
    );

    assert!(
        output.contains("20 FOR EACH S$ IN L:PRINT S$:NEXT EACH\n"),
        "{output}"
    );
    assert!(output.contains("A\nB\n"), "{output}");
}
