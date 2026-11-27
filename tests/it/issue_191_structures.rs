//! Issue #191: a structure groups named members under one name, as in BBC
//! BASIC for Windows. `DIM point{x, y%, name$}` declares it, and
//! `point.x` reads or sets one member.

use crate::common::run_binary_read_first as run_binary;
use crate::common::run_with_limit as run;
use crate::common::run_with_replies;
use bbc_basic_interpreter::tokenizer::{detokenize, tokenize, Token};

// Declare, set and read

#[test]
fn test_declare_set_and_read_the_3_member_types() {
    let (result, output) = run(&[
        "10 DIM point{x, y%, name$}",
        "20 point.x = 1.5 : point.y% = 7 : point.name$ = \"Ann\"",
        "30 PRINT ;point.x;\" \";point.y%;\" \";point.name$",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "1.5 7 Ann\n");
}

#[test]
fn test_new_members_are_zero_and_empty() {
    let (result, output) = run(&[
        "10 DIM point{x, y%, name$}",
        "20 PRINT ;point.x;\"/\";point.y%;\"/\";point.name$;\"/\"",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "0/0//\n");
}

#[test]
fn test_an_integer_member_truncates_a_real_value() {
    let (result, output) = run(&["10 DIM p{n%}", "20 p.n% = 2.7", "30 PRINT ;p.n%"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "2\n");
}

#[test]
fn test_let_sets_a_member() {
    let (result, output) = run(&["10 DIM p{x}", "20 LET p.x = 3.25", "30 PRINT ;p.x"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "3.25\n");
}

#[test]
fn test_a_string_value_in_a_numeric_member_is_a_type_mismatch() {
    let (result, _) = run(&["10 DIM p{x}", "20 p.x = \"A\""]);
    assert_eq!(result, Err("Type mismatch at line 20".to_string()));
}

#[test]
fn test_an_upper_case_name_that_is_not_a_short_form_is_a_structure() {
    let (result, output) = run(&["10 DIM XY{a}", "20 XY.a = 4", "30 PRINT ;XY.a"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "4\n");
}

// A member in each place where a variable is valid

#[test]
fn test_a_member_in_an_expression() {
    let (result, output) = run(&[
        "10 DIM p{x, n%, s$}",
        "20 p.x = 1.5 : p.n% = 4 : p.s$ = \"ab\"",
        "30 PRINT ;p.x * 2 + p.n%;\" \";p.n%-1;\" \";p.s$ + \"c\";\" \";LEN(p.s$)",
        "40 IF p.n% = 4 THEN PRINT \"four\"",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "7 3 abc 2\nfour\n");
}

#[test]
fn test_a_member_in_input() {
    let (result, output) = run_with_replies(
        &[
            "10 DIM p{x, s$}",
            "20 INPUT p.x, p.s$",
            "30 PRINT ;p.x;\" \";p.s$",
        ],
        &["2.5,Bob"],
    );
    assert_eq!(result, Ok(()));
    assert_eq!(output, "?\n2.5 Bob\n");
}

#[test]
fn test_a_member_in_input_line() {
    let (result, output) = run_with_replies(
        &["10 DIM p{s$}", "20 INPUT LINE p.s$", "30 PRINT p.s$"],
        &["a, b"],
    );
    assert_eq!(result, Ok(()));
    assert_eq!(output, "?\na, b\n");
}

#[test]
fn test_a_member_in_read() {
    let (result, output) = run(&[
        "10 DIM p{n%, s$}",
        "20 READ p.n%, p.s$",
        "30 PRINT ;p.n%;\" \";p.s$",
        "40 DATA 12, hello",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "12 hello\n");
}

#[test]
fn test_a_member_in_swap() {
    let (result, output) = run(&[
        "10 DIM p{x}, q{x}",
        "20 p.x = 1 : q.x = 2 : a = 3",
        "30 SWAP p.x, q.x",
        "40 SWAP p.x, a",
        "50 PRINT ;p.x;\" \";q.x;\" \";a",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "3 1 2\n");
}

#[test]
fn test_swap_of_a_string_member_and_a_number_is_a_type_mismatch() {
    let (result, _) = run(&["10 DIM p{s$}", "20 SWAP p.s$, A%"]);
    assert_eq!(result, Err("Type mismatch at line 20".to_string()));
}

// Errors

#[test]
fn test_a_member_that_does_not_exist() {
    let (result, _) = run(&["10 DIM p{x}", "20 PRINT p.z"]);
    assert_eq!(result, Err("No such variable: p.z at line 20".to_string()));
    let (result, _) = run(&["10 DIM p{x}", "20 p.z = 1"]);
    assert_eq!(result, Err("No such variable: p.z at line 20".to_string()));
}

#[test]
fn test_a_member_of_a_name_that_is_not_a_structure() {
    let (result, _) = run(&["10 q = 1", "20 PRINT q.x"]);
    assert_eq!(result, Err("Bad use of structure at line 20".to_string()));
    let (result, _) = run(&["10 q.x = 1"]);
    assert_eq!(result, Err("Bad use of structure at line 10".to_string()));
}

#[test]
fn test_bad_use_of_structure_is_err_56() {
    let (result, output) = run(&["10 ON ERROR PRINT ;ERR : END", "20 PRINT q.x"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "56\n");
}

#[test]
fn test_a_second_dim_of_a_structure() {
    let (result, _) = run(&["10 DIM p{x}", "20 DIM p{x}"]);
    assert_eq!(result, Err("Bad DIM at line 20".to_string()));
}

#[test]
fn test_2_members_with_the_same_name() {
    let (result, _) = run(&["10 DIM p{x, y, x}"]);
    assert_eq!(
        result,
        Err("Syntax error at line 10: Duplicate member name: x".to_string())
    );
}

#[test]
fn test_a_structure_with_no_members() {
    let (result, _) = run(&["10 DIM p{}"]);
    assert_eq!(
        result,
        Err("Syntax error at line 10: Expected member name in DIM".to_string())
    );
}

#[test]
fn test_a_structure_name_that_is_a_short_form() {
    let (result, _) = run(&["10 DIM P{x}"]);
    assert_eq!(
        result,
        Err(
            "Syntax error at line 10: P. is a keyword short form, so P cannot be a structure name"
                .to_string()
        )
    );
}

// The forms of the later child issues give a syntax error

#[test]
fn test_the_forms_of_later_issues_give_a_syntax_error() {
    // Array members and nested structures are valid since issue #192
    let cases: [(&[&str], u16, &str); 3] = [
        (
            &["10 DIM p{(3)a}"],
            10,
            "Arrays of structures are not supported",
        ),
        (
            &["10 DIM a{x}", "20 DIM b{} = a{}"],
            20,
            "A structure from a prototype is not supported",
        ),
        (
            &["10 DIM a{x}, b{x}", "20 b{} = a{}"],
            20,
            "A reference to a whole structure is not supported",
        ),
    ];
    for (source, line, message) in cases {
        let (result, _) = run(source);
        assert_eq!(
            result,
            Err(format!("Syntax error at line {line}: {message}")),
            "{source:?}"
        );
    }
}

#[test]
fn test_local_of_a_member_is_a_syntax_error() {
    let (result, _) = run(&[
        "10 DIM a{x}",
        "20 PROCt",
        "30 END",
        "40 DEF PROCt",
        "50 LOCAL a.x",
        "60 ENDPROC",
    ]);
    assert_eq!(
        result,
        Err("Syntax error at line 50: Expected variable name in LOCAL".to_string())
    );
}

// RUN, NEW and CLEAR remove the structures

#[test]
fn test_clear_removes_the_structures() {
    let (result, _) = run(&["10 DIM p{x}", "20 CLEAR", "30 PRINT p.x"]);
    assert_eq!(result, Err("Bad use of structure at line 30".to_string()));
    let (result, output) = run(&["10 DIM p{x}", "20 CLEAR", "30 DIM p{x}", "40 PRINT ;p.x"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "0\n");
}

#[test]
fn test_a_second_run_declares_the_structure_again() {
    let output = run_binary(
        "10 DIM p{x}\n20 p.x = p.x + 5\n30 PRINT \"v\";p.x\nRUN\nRUN\nNEW\nPRINT p.x\nEXIT\n",
    );
    assert_eq!(output.matches("v5").count(), 2, "output: {output}");
    assert!(!output.contains("Bad DIM"), "output: {output}");
    assert!(output.contains("Bad use of structure"), "output: {output}");
}

#[test]
fn test_lvar_lists_each_member() {
    let output = run_binary(
        "10 DIM pt{x, n%, s$}\n20 pt.x = 1.5 : pt.n% = 3 : pt.s$ = \"A\"\nRUN\nLVAR\nEXIT\n",
    );
    for line in ["pt.x = 1.5", "pt.n% = 3", "pt.s$ = \"A\""] {
        assert!(output.contains(line), "{line} in output: {output}");
    }
}

// The other meanings of the dot, and the other kinds of the name

#[test]
fn test_short_forms_and_numbers_are_unchanged() {
    let (result, output) = run(&["10 P.\"A\";1.5;.5", "20 IF 1 A. 1 THEN P.\"B\""]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "A1.50.5\nB\n");
}

#[test]
fn test_a_short_form_before_a_name_is_the_keyword() {
    // P.x is PRINT x, not the member x of a structure P
    let (result, output) = run(&["10 x = 3", "20 P.;x"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "3\n");
}

#[test]
fn test_a_variable_an_array_and_a_structure_with_one_name() {
    let (result, output) = run(&[
        "10 DIM point(2), point{x}",
        "20 point = 1 : point(1) = 2 : point.x = 3",
        "30 PRINT ;point;point(1);point.x",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "123\n");
}

#[test]
fn test_tokens_of_a_member_reference() {
    let tokens = tokenize("point.name$ = 1").unwrap().tokens;
    assert_eq!(
        tokens,
        vec![
            Token::Member("point".to_string(), vec!["name$".to_string()]),
            Token::Operator('='),
            Token::Integer(1),
        ]
    );
}

#[test]
fn test_list_shows_the_lines_as_typed() {
    for line in [
        "10 DIM point{x,y%,name$}",
        "20 point.x = 1.5",
        "30 PRINT point.name$;point.x - 1",
    ] {
        let listed = detokenize(&tokenize(line).unwrap()).unwrap();
        assert_eq!(listed, line);
    }
}

// The corrections of the code review

/// Run the PRINT line with members, and with variables of the same values,
/// and give the 2 outputs. The PRINT line uses `p.` before each name.
fn print_member_and_variable(print: &str) -> (String, String) {
    let (result, member_output) = run(&[
        "10 DIM p{x, n%, s$}",
        "20 p.x = 1.5 : p.n% = 7 : p.s$ = \"ab\"",
        &format!("30 {print}"),
    ]);
    assert_eq!(result, Ok(()), "{print}");
    let (result, variable_output) = run(&[
        "10 x = 1.5 : n% = 7 : s$ = \"ab\"",
        &format!("30 {}", print.replace("p.", "")),
    ]);
    assert_eq!(result, Ok(()), "{print}");
    (member_output, variable_output)
}

#[test]
fn test_print_a_member_after_an_item_with_no_separator() {
    for print in ["PRINT \"x=\" p.x", "PRINT p.x p.n%", "PRINT \"s=\" p.s$"] {
        let (member, variable) = print_member_and_variable(print);
        assert_eq!(member, variable, "{print}");
    }
}

#[test]
fn test_print_a_member_in_a_field_as_a_variable() {
    for print in ["PRINT p.n%", "PRINT p.x", "PRINT p.s$", "PRINT p.n%, p.x"] {
        let (member, variable) = print_member_and_variable(print);
        assert_eq!(member, variable, "{print}");
    }
    let (_, output) = print_member_and_variable("PRINT p.n%");
    assert_eq!(output, "         7\n");
}

#[test]
fn test_input_checks_the_member_before_the_reply() {
    for input in ["20 INPUT p.z", "20 INPUT LINE p.z$"] {
        let (result, output) = run_with_replies(&["10 DIM p{x}", input], &["5"]);
        let name = input.rsplit(' ').next().unwrap();
        assert_eq!(
            result,
            Err(format!("No such variable: {name} at line 20")),
            "{input}"
        );
        assert_eq!(output, "", "{input}");
    }
}

#[test]
fn test_read_checks_the_member_before_it_takes_a_value() {
    let (result, output) = run(&[
        "10 ON ERROR READ a : PRINT ;a : END",
        "20 READ q.x",
        "30 DATA 7",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "7\n");
}

#[test]
fn test_input_file_checks_the_member_before_it_takes_a_record() {
    let dir = std::env::temp_dir().join(format!("bbc_issue_191_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir is created");
    let file = dir.join("data.txt").to_string_lossy().into_owned();
    let (result, output) = run(&[
        &format!("10 H = OPENOUT(\"{file}\")"),
        "20 PRINT#H, \"first\" : CLOSE#H",
        &format!("30 H = OPENIN(\"{file}\")"),
        "40 ON ERROR INPUT#H, a$ : PRINT a$ : CLOSE#H : END",
        "50 INPUT#H, q.x",
    ]);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "first\n");
}

#[test]
fn test_a_member_name_can_start_with_an_underscore() {
    let (result, output) = run(&["10 DIM p{_a}", "20 p._a = 2", "30 PRINT ;p._a"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "2\n");
    // A nested member is valid since issue #192
    for (text, last) in [("PRINT a.b.c", "c"), ("PRINT a.b._c", "_c")] {
        let tokens = tokenize(text).unwrap().tokens;
        let path = vec!["b".to_string(), last.to_string()];
        assert_eq!(tokens[1], Token::Member("a".to_string(), path), "{text}");
    }
}
