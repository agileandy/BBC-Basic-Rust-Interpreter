//! Issue 139: lists that grow. NEWLIST, LISTLEN, LISTGET, LISTGET$,
//! LISTISSTR, LISTADD, LISTSET, LISTINSERT, LISTREMOVE and LISTCLEAR are
//! modern extensions, not BBC BASIC. Old programs that use these names for
//! variables and arrays still work.

use crate::common::{run_binary_read_first, run_lines as run};

#[test]
fn test_listadd_adds_numbers_and_strings_and_listlen_counts_them() {
    let (result, output) = run(&[
        "10 L% = NEWLIST",
        "20 PRINT LISTLEN(L%)",
        "30 LISTADD L%, 5",
        "40 LISTADD L%, \"CAT\"",
        "50 LISTADD L%, 2.5",
        "60 PRINT LISTLEN(L%)",
        "70 PRINT LISTGET(L%, 0)",
        "80 PRINT LISTGET$(L%, 1)",
        "90 PRINT LISTGET(L%, 2)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(
        output,
        "         0\n         3\n         5\nCAT\n       2.5\n"
    );
}

#[test]
fn test_listisstr_is_true_only_for_a_string_element() {
    let (result, output) = run(&[
        "10 L = NEWLIST",
        "20 LISTADD L, 1",
        "30 LISTADD L, \"A\" + \"B\"",
        "40 PRINT LISTISSTR(L, 0)",
        "50 PRINT LISTISSTR(L, 1)",
        "60 PRINT LISTGET$(L, 1)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         0\n        -1\nAB\n");
}

#[test]
fn test_listset_changes_an_element_and_its_type() {
    let (result, output) = run(&[
        "10 L% = NEWLIST",
        "20 LISTADD L%, 1",
        "30 LISTADD L%, 2",
        "40 LISTSET L%, 0, \"ONE\"",
        "50 LISTSET L%, 1, 20",
        "60 PRINT LISTGET$(L%, 0)",
        "70 PRINT LISTGET(L%, 1)",
        "80 PRINT LISTLEN(L%)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "ONE\n        20\n         2\n");
}

#[test]
fn test_listinsert_puts_an_element_before_the_index_or_at_the_end() {
    let (result, output) = run(&[
        "10 L% = NEWLIST",
        "20 LISTADD L%, 2",
        "30 LISTINSERT L%, 0, 1",
        "40 LISTINSERT L%, 2, 3",
        "50 FOR I% = 0 TO LISTLEN(L%) - 1",
        "60 PRINT LISTGET(L%, I%)",
        "70 NEXT",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         1\n         2\n         3\n");
}

#[test]
fn test_listremove_removes_one_element_and_listclear_removes_all() {
    let (result, output) = run(&[
        "10 L% = NEWLIST",
        "20 LISTADD L%, 1",
        "30 LISTADD L%, 2",
        "40 LISTADD L%, 3",
        "50 LISTREMOVE L%, 1",
        "60 PRINT LISTGET(L%, 0); LISTGET(L%, 1); LISTLEN(L%)",
        "70 LISTCLEAR L%",
        "80 PRINT LISTLEN(L%)",
        "90 LISTADD L%, 9",
        "100 PRINT LISTGET(L%, 0)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         132\n         0\n         9\n");
}

#[test]
fn test_newlist_with_empty_brackets_gives_a_different_handle() {
    let (result, output) = run(&[
        "10 A% = NEWLIST",
        "20 B% = NEWLIST()",
        "30 PRINT A% > 0; B% > 0; A% <> B%",
        "40 LISTADD A%, 1",
        "50 PRINT LISTLEN(A%); LISTLEN(B%)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "        -1-1-1\n         10\n");
}

#[test]
fn test_a_list_holds_the_handle_of_a_list() {
    let (result, output) = run(&[
        "10 OUTER% = NEWLIST",
        "20 INNER% = NEWLIST",
        "30 LISTADD INNER%, \"DEEP\"",
        "40 LISTADD OUTER%, INNER%",
        "50 PRINT LISTGET$(LISTGET(OUTER%, 0), 0)",
        "60 H% = LISTGET(OUTER%, 0)",
        "70 PRINT LISTLEN(H%)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "DEEP\n         1\n");
}

#[test]
fn test_a_comma_in_brackets_does_not_divide_the_statement_arguments() {
    let (result, output) = run(&[
        "10 A% = NEWLIST: B% = NEWLIST",
        "20 LISTADD B%, \"X\": LISTADD B%, 7",
        "30 LISTADD A%, LISTGET(B%, 1)",
        "40 LISTINSERT A%, LISTLEN(A%) - 1, LISTGET$(B%, 0)",
        "50 PRINT LISTGET$(A%, 0); LISTGET(A%, 1)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "X7\n");
}

#[test]
fn test_the_wrong_count_of_arguments_gives_a_syntax_error() {
    let (result, _) = run(&["10 L% = NEWLIST", "20 LISTADD L%"]);
    assert_eq!(
        result,
        Err("Syntax error at line 20: LISTADD requires 2 arguments, got 1".to_string())
    );

    let (result, _) = run(&["10 L% = NEWLIST", "20 PRINT LISTGET(L%)"]);
    assert_eq!(
        result,
        Err("Syntax error at line 20: LISTGET requires 2 arguments".to_string())
    );
}

#[test]
fn test_a_handle_that_is_not_a_list_gives_bad_collection() {
    let (result, output) = run(&["10 PRINT LISTLEN(7)"]);

    assert_eq!(result, Err("Bad collection at line 10".to_string()));
    assert_eq!(output, "");
}

/// Run one list statement after a list L% with 2 elements (5 and "A") is
/// made, with ON ERROR, and return what the handler prints.
fn caught(statement: &str) -> String {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 L% = NEWLIST: LISTADD L%, 5: LISTADD L%, \"A\"",
        &format!("30 {}", statement),
        "40 PRINT \"NOT CAUGHT\"",
        "50 END",
        "100 PRINT REPORT$; \" \"; ERR; \" \"; ERL",
    ]);
    assert_eq!(result, Ok(()));
    output
}

#[test]
fn test_on_error_catches_bad_collection_with_err_153() {
    // ERR 153 is Bad collection. BBC BASIC does not use 153.
    let expected = "Bad collection 153 30\n";
    assert_eq!(caught("PRINT LISTLEN(0)"), expected);
    assert_eq!(caught("PRINT LISTGET(99, 0)"), expected);
    assert_eq!(caught("PRINT LISTGET$(99, 0)"), expected);
    assert_eq!(caught("PRINT LISTISSTR(99, 0)"), expected);
    assert_eq!(caught("LISTADD 99, 1"), expected);
    assert_eq!(caught("LISTSET 99, 0, 1"), expected);
    assert_eq!(caught("LISTINSERT 99, 0, 1"), expected);
    assert_eq!(caught("LISTREMOVE 99, 0"), expected);
    assert_eq!(caught("LISTCLEAR 99"), expected);
}

#[test]
fn test_on_error_catches_an_index_out_of_range_with_err_15() {
    let expected = "Subscript 15 30\n";
    assert_eq!(caught("PRINT LISTGET(L%, 2)"), expected);
    assert_eq!(caught("PRINT LISTGET$(L%, -1)"), expected);
    assert_eq!(caught("PRINT LISTISSTR(L%, 2)"), expected);
    assert_eq!(caught("LISTSET L%, 2, 1"), expected);
    assert_eq!(caught("LISTINSERT L%, 3, 1"), expected);
    assert_eq!(caught("LISTREMOVE L%, 2"), expected);
}

#[test]
fn test_on_error_catches_the_wrong_getter_with_err_6() {
    let expected = "Type mismatch 6 30\n";
    assert_eq!(caught("PRINT LISTGET(L%, 1)"), expected);
    assert_eq!(caught("PRINT LISTGET$(L%, 0)"), expected);
}

#[test]
fn test_a_second_run_starts_with_no_lists() {
    // L% is a resident integer variable: it keeps the handle after RUN.
    let output = run_binary_read_first(
        "10 IF L% = 0 THEN L% = NEWLIST: LISTADD L%, 5: PRINT \"MADE\": END\n\
         20 PRINT LISTLEN(L%)\n\
         RUN\n\
         RUN\n\
         EXIT\n",
    );

    assert!(output.contains("MADE"), "output: {}", output);
    assert!(
        output.contains("Bad collection at line 20"),
        "output: {}",
        output
    );
}

#[test]
fn test_clear_removes_all_lists() {
    let (result, output) = run(&["10 L% = NEWLIST", "20 CLEAR", "30 PRINT LISTLEN(L%)"]);

    assert_eq!(result, Err("Bad collection at line 30".to_string()));
    assert_eq!(output, "");
}

#[test]
fn test_each_statement_name_is_still_a_variable_and_an_array() {
    for name in [
        "LISTADD",
        "LISTSET",
        "LISTINSERT",
        "LISTREMOVE",
        "LISTCLEAR",
    ] {
        let (result, output) = run(&[
            &format!("10 {} = 3", name),
            &format!("20 DIM {}(2)", name),
            &format!("30 {}(1) = 4", name),
            &format!("40 PRINT {} + {}(1)", name, name),
        ]);

        assert_eq!(result, Ok(()), "name: {}", name);
        assert_eq!(output, "         7\n", "name: {}", name);
    }
}

#[test]
fn test_each_function_name_is_still_a_variable_and_an_array() {
    for name in ["NEWLIST", "LISTLEN", "LISTGET", "LISTISSTR"] {
        let (result, output) = run(&[
            &format!("10 {} = 3", name),
            &format!("20 DIM {}(2)", name),
            &format!("30 {}(1) = 4", name),
            &format!("40 PRINT {} + {}(1)", name, name),
        ]);

        assert_eq!(result, Ok(()), "name: {}", name);
        assert_eq!(output, "         7\n", "name: {}", name);
    }

    let (result, output) = run(&[
        "10 LISTGET$ = \"A\"",
        "20 DIM LISTGET$(2)",
        "30 LISTGET$(1) = \"B\"",
        "40 PRINT LISTGET$ + LISTGET$(1)",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "AB\n");
}
