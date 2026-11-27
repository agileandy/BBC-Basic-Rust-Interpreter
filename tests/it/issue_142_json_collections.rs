//! Issue 142: conversion between JSON text and lists and maps. JSONTOLIST,
//! JSONTOMAP, TOJSON$, LISTKIND$, MAPKIND$, LISTADDCOL and MAPPUTCOL are
//! modern extensions, not BBC BASIC. Old programs that use these names for
//! variables and arrays still work.

use crate::common::{run_binary_read_first, run_timed_ran_for as run};

/// A BASIC string literal for a text: a quote in the text becomes 2 quotes.
fn literal(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "\"\""))
}

/// A nested document with each JSON type, in compact form.
const DOCUMENT: &str = r#"{"name":"Ann \"A\"\nB","age":42,"score":-1.5,"ok":true,"no":false,"none":null,"tags":["a",1,[],{}],"inner":{"x":[true,null,{"y":"z"}]}}"#;

#[test]
fn test_a_round_trip_of_a_nested_map_gives_the_same_text() {
    let (result, output) = run(&[
        &format!("10 J$ = {}", literal(DOCUMENT)),
        "20 M% = JSONTOMAP(J$)",
        "30 PRINT TOJSON$(M%)",
        "40 PRINT TOJSON$(M%) = J$",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, format!("{}\n        -1\n", DOCUMENT));
}

#[test]
fn test_a_round_trip_of_a_nested_list_gives_the_same_text() {
    let text = r#"[1,2.5,-3,"s",true,false,null,[[],[1,[2]]],{"k":{"m":[]}}]"#;
    let (result, output) = run(&[
        &format!("10 L% = JSONTOLIST({})", literal(text)),
        "20 PRINT TOJSON$(L%)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, format!("{}\n", text));
}

#[test]
fn test_the_getters_read_the_converted_elements() {
    let (result, output) = run(&[
        &format!(
            "10 L% = JSONTOLIST({})",
            literal(r#"[7,"A",[8,9],{"k":"v"},true,false,null]"#)
        ),
        "20 PRINT LISTLEN(L%); \" \"; LISTGET(L%, 0); \" \"; LISTGET$(L%, 1)",
        "30 N% = LISTGET(L%, 2)",
        "40 PRINT LISTLEN(N%); \" \"; LISTGET(N%, 1)",
        "50 M% = LISTGET(L%, 3)",
        "60 PRINT MAPGET$(M%, \"k\")",
        "70 PRINT LISTGET(L%, 4); \" \"; LISTGET(L%, 5); \" \"; LISTGET(L%, 6)",
        "80 PRINT LISTISSTR(L%, 2); \" \"; LISTISSTR(L%, 4); \" \"; LISTISSTR(L%, 6)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(
        output,
        "         7 7 A\n         2 9\nv\n        -1 0 0\n         0 0 0\n"
    );
}

#[test]
fn test_listkind_and_mapkind_give_the_kind_of_each_element() {
    let (result, output) = run(&[
        &format!(
            "10 L% = JSONTOLIST({})",
            literal(r#"[1,"A",[],{},true,null]"#)
        ),
        "20 FOR I% = 0 TO LISTLEN(L%) - 1",
        "30 PRINT LISTKIND$(L%, I%)",
        "40 NEXT",
        &format!(
            "50 M% = JSONTOMAP({})",
            literal(r#"{"n":1,"s":"A","l":[],"m":{},"b":false,"z":null}"#)
        ),
        "60 FOR I% = 0 TO MAPLEN(M%) - 1",
        "70 PRINT MAPKIND$(M%, MAPKEY$(M%, I%))",
        "80 NEXT",
    ]);

    let kinds = "number\nstring\nlist\nmap\nboolean\nnull\n";
    assert_eq!(result, Ok(()));
    assert_eq!(output, format!("{}{}", kinds, kinds));
}

#[test]
fn test_listaddcol_and_mapputcol_put_a_nested_collection_and_listadd_puts_a_number() {
    let (result, output) = run(&[
        "10 L% = NEWLIST",
        "20 M% = NEWMAP",
        "30 MAPPUT M%, \"k\", 1",
        "40 T% = NEWLIST",
        "50 LISTADD T%, 3",
        "60 LISTADD T%, \"X\"",
        "70 MAPPUTCOL M%, \"t\", T%",
        "80 LISTADDCOL L%, M%",
        "90 LISTADD L%, M%",
        "100 PRINT TOJSON$(L%)",
        "110 PRINT LISTKIND$(L%, 0); \" \"; LISTKIND$(L%, 1); \" \"; MAPKIND$(M%, \"t\")",
        "120 PRINT LISTGET(L%, 0) = M%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(
        output,
        "[{\"k\":1,\"t\":[3,\"X\"]},2]\nmap number list\n        -1\n"
    );
}

#[test]
fn test_a_number_with_no_fraction_has_no_decimal_point() {
    let (result, output) = run(&[
        "10 L% = NEWLIST",
        "20 LISTADD L%, 3",
        "30 LISTADD L%, 6 / 2",
        "40 LISTADD L%, 2.5",
        "50 LISTADD L%, -0.25",
        "60 PRINT TOJSON$(L%)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "[3,3,2.5,-0.25]\n");
}

#[test]
fn test_an_empty_list_and_an_empty_map_give_empty_json() {
    let (result, output) = run(&["10 PRINT TOJSON$(NEWLIST); TOJSON$(NEWMAP)"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "[]{}\n");
}

#[test]
fn test_a_list_that_holds_itself_stops_tojson_with_bad_collection() {
    let (result, output) = run(&[
        "10 L% = NEWLIST",
        "20 M% = NEWMAP",
        "30 LISTADDCOL L%, M%",
        "40 MAPPUTCOL M%, \"back\", L%",
        "50 PRINT TOJSON$(L%)",
    ]);

    assert_eq!(result, Err("Bad collection at line 50".to_string()));
    assert_eq!(output, "");
}

/// Run one statement after a list L% and a map M% are made, with ON ERROR,
/// and return what the handler prints.
fn caught(statement: &str) -> String {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 L% = NEWLIST: M% = NEWMAP: MAPPUT M%, \"N\", 5",
        &format!("30 {}", statement),
        "40 PRINT \"NOT CAUGHT\"",
        "50 END",
        "100 PRINT REPORT$; \" \"; ERR; \" \"; ERL",
    ]);
    assert_eq!(result, Ok(()));
    output
}

#[test]
fn test_on_error_catches_a_root_of_the_wrong_type_with_err_6() {
    let expected = "Type mismatch 6 30\n";
    assert_eq!(caught("X% = JSONTOLIST(\"{}\")"), expected);
    assert_eq!(caught("X% = JSONTOLIST(\"3\")"), expected);
    assert_eq!(caught("X% = JSONTOMAP(\"[]\")"), expected);
    assert_eq!(caught("X% = JSONTOMAP(\"\"\"A\"\"\")"), expected);
}

#[test]
fn test_on_error_catches_a_text_that_is_not_json_with_err_150() {
    assert_eq!(
        caught("X% = JSONTOLIST(\"[1,\")"),
        "Bad JSON: EOF while parsing a value at line 1 column 3 150 30\n"
    );
    assert_eq!(
        caught("X% = JSONTOMAP(\"\")"),
        "Bad JSON: EOF while parsing a value at line 1 column 0 150 30\n"
    );
}

#[test]
fn test_on_error_catches_a_result_that_is_too_long_with_err_19() {
    let expected = "String too long 19 30\n";
    assert_eq!(
        caught("LISTADD L%, STRING$(40000, \"A\"): LISTADD L%, STRING$(40000, \"B\"): PRINT TOJSON$(L%)"),
        expected
    );
}

#[test]
fn test_on_error_catches_bad_collection_with_err_153() {
    let expected = "Bad collection 153 30\n";
    assert_eq!(caught("PRINT TOJSON$(99)"), expected);
    assert_eq!(caught("LISTADDCOL L%, 99"), expected);
    assert_eq!(caught("LISTADDCOL 99, M%"), expected);
    assert_eq!(caught("MAPPUTCOL M%, \"K\", 99"), expected);
    assert_eq!(caught("MAPPUTCOL L%, \"K\", M%"), expected);
    assert_eq!(caught("PRINT LISTKIND$(M%, 0)"), expected);
    assert_eq!(caught("PRINT MAPKIND$(L%, \"N\")"), expected);
    assert_eq!(caught("LISTADDCOL L%, L%: PRINT TOJSON$(L%)"), expected);
}

#[test]
fn test_on_error_catches_a_wrong_index_or_key_of_a_kind_function() {
    assert_eq!(caught("PRINT LISTKIND$(L%, 0)"), "Subscript 15 30\n");
    assert_eq!(
        caught("PRINT MAPKIND$(M%, \"X\")"),
        "No such key: X 154 30\n"
    );
}

#[test]
fn test_each_statement_name_is_still_a_variable_and_an_array() {
    for name in ["LISTADDCOL", "MAPPUTCOL"] {
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
    for name in ["JSONTOLIST", "JSONTOMAP"] {
        let (result, output) = run(&[
            &format!("10 {} = 3", name),
            &format!("20 DIM {}(2)", name),
            &format!("30 {}(1) = 4", name),
            &format!("40 PRINT {} + {}(1)", name, name),
        ]);

        assert_eq!(result, Ok(()), "name: {}", name);
        assert_eq!(output, "         7\n", "name: {}", name);
    }

    for name in ["TOJSON$", "LISTKIND$", "MAPKIND$"] {
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

#[test]
fn test_binary_runs_a_program_with_a_round_trip() {
    let input = "10 M% = JSONTOMAP(\"{\"\"a\"\":[1,true,null],\"\"b\"\":{\"\"c\"\":\"\"d\"\"}}\")\n20 MAPPUT M%, \"e\", 2.5\n30 PRINT TOJSON$(M%)\nRUN\nEXIT\n";
    let output = run_binary_read_first(input);
    assert!(
        output.contains("{\"a\":[1,true,null],\"b\":{\"c\":\"d\"},\"e\":2.5}\n"),
        "output: {output}"
    );
}
