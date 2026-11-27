//! Issue 140: maps from a text key to a value. NEWMAP, MAPGET, MAPGET$,
//! MAPHAS, MAPLEN, MAPKEY$, MAPISSTR, MAPPUT, MAPREMOVE and MAPCLEAR are
//! modern extensions, not BBC BASIC. Old programs that use these names for
//! variables and arrays still work.

use crate::common::run_lines as run;

#[test]
fn test_mapput_puts_numbers_and_strings_and_mapget_reads_them() {
    let (result, output) = run(&[
        "10 M% = NEWMAP",
        "20 PRINT MAPLEN(M%)",
        "30 MAPPUT M%, \"AGE\", 42",
        "40 MAPPUT M%, \"NAME\", \"ANN\"",
        "50 PRINT MAPLEN(M%)",
        "60 PRINT MAPGET(M%, \"AGE\")",
        "70 PRINT MAPGET$(M%, \"NAME\")",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         0\n         2\n        42\nANN\n");
}

#[test]
fn test_mapput_of_a_key_that_is_there_replaces_the_value_and_keeps_the_order() {
    let (result, output) = run(&[
        "10 M% = NEWMAP",
        "20 MAPPUT M%, \"B\", 1",
        "30 MAPPUT M%, \"A\", 2",
        "40 MAPPUT M%, \"C\", 3",
        "50 MAPPUT M%, \"B\", \"ONE\"",
        "60 FOR I% = 0 TO MAPLEN(M%) - 1",
        "70 PRINT MAPKEY$(M%, I%)",
        "80 NEXT",
        "90 PRINT MAPGET$(M%, \"B\"); MAPLEN(M%)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "B\nA\nC\nONE3\n");
}

#[test]
fn test_maphas_and_mapisstr_tell_about_a_key() {
    let (result, output) = run(&[
        "10 M% = NEWMAP",
        "20 MAPPUT M%, \"N\", 1",
        "30 MAPPUT M%, \"S\", \"A\" + \"B\"",
        "40 PRINT MAPHAS(M%, \"N\"); MAPHAS(M%, \"X\"); MAPHAS(M%, \"n\")",
        "50 PRINT MAPISSTR(M%, \"N\"); MAPISSTR(M%, \"S\")",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "        -100\n         0-1\n");
}

#[test]
fn test_upper_and_lower_case_keys_are_different_and_an_empty_key_is_valid() {
    let (result, output) = run(&[
        "10 M% = NEWMAP",
        "20 MAPPUT M%, \"K\", 1",
        "30 MAPPUT M%, \"k\", 2",
        "40 MAPPUT M%, \"\", 3",
        "50 PRINT MAPGET(M%, \"K\"); MAPGET(M%, \"k\"); MAPGET(M%, \"\"); MAPLEN(M%)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         1233\n");
}

#[test]
fn test_mapremove_keeps_the_order_of_the_other_keys_and_mapclear_removes_all() {
    let (result, output) = run(&[
        "10 M% = NEWMAP",
        "20 MAPPUT M%, \"A\", 1: MAPPUT M%, \"B\", 2: MAPPUT M%, \"C\", 3",
        "30 MAPREMOVE M%, \"A\"",
        "40 MAPPUT M%, \"A\", 4",
        "50 FOR I% = 0 TO MAPLEN(M%) - 1",
        "60 PRINT MAPKEY$(M%, I%); MAPGET(M%, MAPKEY$(M%, I%))",
        "70 NEXT",
        "80 MAPCLEAR M%",
        "90 PRINT MAPLEN(M%); MAPHAS(M%, \"B\")",
        "100 MAPPUT M%, \"Z\", 9",
        "110 PRINT MAPKEY$(M%, 0)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "B2\nC3\nA4\n         00\nZ\n");
}

#[test]
fn test_newmap_with_empty_brackets_gives_a_different_handle() {
    let (result, output) = run(&[
        "10 A% = NEWMAP",
        "20 B% = NEWMAP()",
        "30 L% = NEWLIST",
        "40 PRINT A% > 0; B% > 0; A% <> B%; B% <> L%",
        "50 MAPPUT A%, \"K\", 1",
        "60 PRINT MAPLEN(A%); MAPLEN(B%)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "        -1-1-1-1\n         10\n");
}

#[test]
fn test_a_map_in_a_list_and_a_list_in_a_map() {
    let (result, output) = run(&[
        "10 L% = NEWLIST: M% = NEWMAP",
        "20 MAPPUT M%, \"NAME\", \"ANN\"",
        "30 LISTADD L%, M%",
        "40 PRINT MAPGET$(LISTGET(L%, 0), \"NAME\")",
        "50 P% = NEWLIST: LISTADD P%, 7",
        "60 MAPPUT M%, \"ITEMS\", P%",
        "70 PRINT LISTGET(MAPGET(M%, \"ITEMS\"), 0)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "ANN\n         7\n");
}

#[test]
fn test_a_comma_in_brackets_does_not_divide_the_statement_arguments() {
    let (result, output) = run(&[
        "10 M% = NEWMAP: N% = NEWMAP",
        "20 MAPPUT N%, \"K\", \"KEY\"",
        "30 MAPPUT M%, MAPGET$(N%, \"K\"), MAPLEN(N%)",
        "40 PRINT MAPKEY$(M%, 0); MAPGET(M%, \"KEY\")",
        "50 MAPREMOVE M%, MAPGET$(N%, \"K\")",
        "60 PRINT MAPLEN(M%)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "KEY1\n         0\n");
}

#[test]
fn test_the_wrong_count_of_arguments_gives_a_syntax_error() {
    let (result, _) = run(&["10 M% = NEWMAP", "20 MAPPUT M%, \"K\""]);
    assert_eq!(
        result,
        Err("Syntax error at line 20: MAPPUT requires 3 arguments, got 2".to_string())
    );

    let (result, _) = run(&["10 M% = NEWMAP", "20 PRINT MAPGET$(M%)"]);
    assert_eq!(
        result,
        Err("Syntax error at line 20: MAPGET$ requires 2 arguments".to_string())
    );
}

#[test]
fn test_a_key_that_is_not_there_stops_the_program_with_no_such_key() {
    let (result, output) = run(&["10 M% = NEWMAP", "20 PRINT MAPGET(M%, \"AGE\")"]);

    assert_eq!(result, Err("No such key: AGE at line 20".to_string()));
    assert_eq!(output, "");
}

/// Run one map statement after a map M% with 2 keys ("N" is 5 and "S" is
/// "A") and a list L% are made, with ON ERROR, and return what the handler
/// prints.
fn caught(statement: &str) -> String {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 M% = NEWMAP: MAPPUT M%, \"N\", 5: MAPPUT M%, \"S\", \"A\": L% = NEWLIST",
        &format!("30 {}", statement),
        "40 PRINT \"NOT CAUGHT\"",
        "50 END",
        "100 PRINT REPORT$; \" \"; ERR; \" \"; ERL",
    ]);
    assert_eq!(result, Ok(()));
    output
}

#[test]
fn test_on_error_catches_no_such_key_with_err_154() {
    // ERR 154 is No such key. BBC BASIC does not use 154.
    let expected = "No such key: X 154 30\n";
    assert_eq!(caught("PRINT MAPGET(M%, \"X\")"), expected);
    assert_eq!(caught("PRINT MAPGET$(M%, \"X\")"), expected);
    assert_eq!(caught("PRINT MAPISSTR(M%, \"X\")"), expected);
    assert_eq!(caught("MAPREMOVE M%, \"X\""), expected);
}

#[test]
fn test_on_error_catches_bad_collection_with_err_153() {
    let expected = "Bad collection 153 30\n";
    assert_eq!(caught("PRINT MAPLEN(99)"), expected);
    assert_eq!(caught("PRINT MAPGET(99, \"N\")"), expected);
    assert_eq!(caught("PRINT MAPGET$(99, \"S\")"), expected);
    assert_eq!(caught("PRINT MAPHAS(99, \"N\")"), expected);
    assert_eq!(caught("PRINT MAPISSTR(99, \"N\")"), expected);
    assert_eq!(caught("PRINT MAPKEY$(99, 0)"), expected);
    assert_eq!(caught("MAPPUT 99, \"N\", 1"), expected);
    assert_eq!(caught("MAPREMOVE 99, \"N\""), expected);
    assert_eq!(caught("MAPCLEAR 99"), expected);
}

#[test]
fn test_a_map_function_on_a_list_and_a_list_function_on_a_map_give_bad_collection() {
    let expected = "Bad collection 153 30\n";
    assert_eq!(caught("PRINT MAPLEN(L%)"), expected);
    assert_eq!(caught("MAPPUT L%, \"N\", 1"), expected);
    assert_eq!(caught("PRINT LISTLEN(M%)"), expected);
    assert_eq!(caught("LISTADD M%, 1"), expected);
}

#[test]
fn test_on_error_catches_a_key_index_out_of_range_with_err_15() {
    let expected = "Subscript 15 30\n";
    assert_eq!(caught("PRINT MAPKEY$(M%, 2)"), expected);
    assert_eq!(caught("PRINT MAPKEY$(M%, -1)"), expected);
}

#[test]
fn test_on_error_catches_the_wrong_getter_with_err_6() {
    let expected = "Type mismatch 6 30\n";
    assert_eq!(caught("PRINT MAPGET(M%, \"S\")"), expected);
    assert_eq!(caught("PRINT MAPGET$(M%, \"N\")"), expected);
}

#[test]
fn test_clear_removes_all_maps() {
    let (result, output) = run(&["10 M% = NEWMAP", "20 CLEAR", "30 PRINT MAPLEN(M%)"]);

    assert_eq!(result, Err("Bad collection at line 30".to_string()));
    assert_eq!(output, "");
}

#[test]
fn test_each_statement_name_is_still_a_variable_and_an_array() {
    for name in ["MAPPUT", "MAPREMOVE", "MAPCLEAR"] {
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
    for name in ["NEWMAP", "MAPLEN", "MAPGET", "MAPHAS", "MAPISSTR"] {
        let (result, output) = run(&[
            &format!("10 {} = 3", name),
            &format!("20 DIM {}(2)", name),
            &format!("30 {}(1) = 4", name),
            &format!("40 PRINT {} + {}(1)", name, name),
        ]);

        assert_eq!(result, Ok(()), "name: {}", name);
        assert_eq!(output, "         7\n", "name: {}", name);
    }

    for name in ["MAPGET$", "MAPKEY$"] {
        let (result, output) = run(&[
            &format!("10 {} = \"A\"", name),
            &format!("20 DIM {}(2)", name),
            &format!("30 {}(1) = \"B\"", name),
            &format!("40 PRINT {} + {}(1)", name, name),
        ]);

        assert_eq!(result, Ok(()), "name: {}", name);
        assert_eq!(output, "AB\n", "name: {}", name);
    }
}
