//! Issue #192: a structure member can be an array or a structure, as in BBC
//! BASIC for Windows. `DIM layer{weights(20, 20), biases(20)}` declares array
//! members, and `DIM shape{pos{x, y}}` declares a nested structure.

use crate::common::run_binary_read_first as run_binary;
use crate::common::run_with_limit as run;
use crate::common::run_with_replies;
use bbc_basic_interpreter::tokenizer::{detokenize, tokenize, Token};

// Array members

#[test]
fn test_an_array_member_with_1_dimension_for_the_3_types() {
    let (result, output) = run(&[
        "10 DIM s{a(3), n%(3), t$(3)}",
        "20 FOR I% = 0 TO 3 : s.a(I%) = I% / 2 : s.n%(I%) = I% * 3 : s.t$(I%) = STRING$(I%, \"x\") : NEXT",
        "30 PRINT ;s.a(3);\" \";s.n%(2);\" \";s.t$(3);\"/\";s.t$(0);\"/\"",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "1.5 6 xxx//\n");
}

#[test]
fn test_an_array_member_with_2_dimensions_for_the_3_types() {
    let (result, output) = run(&[
        "10 DIM m{a(2, 3), n%(2, 3), t$(2, 3)}",
        "20 m.a(2, 3) = 0.5 : m.n%(1, 2) = 7.9 : m.t$(0, 3) = \"top\"",
        "30 PRINT ;m.a(2, 3);\" \";m.n%(1, 2);\" \";m.t$(0, 3);\" \";m.a(1, 1)",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "0.5 7 top 0\n");
}

#[test]
fn test_the_size_of_an_array_member_can_be_an_expression() {
    let (result, output) = run(&[
        "10 N% = 4",
        "20 DIM s{a(N% * 2)}",
        "30 s.a(8) = 1 : PRINT ;s.a(8)",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n");
}

#[test]
fn test_the_layer_example_of_the_issue() {
    let (result, output) = run(&[
        "10 DIM layer{weights(20, 20), biases(20), activation$}",
        "20 layer.weights(1, 2) = 0.5 : layer.biases(20) = -1",
        "30 layer.activation$ = \"relu\"",
        "40 PRINT ;layer.weights(1, 2) + layer.biases(20);\" \";layer.activation$",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "-0.5 relu\n");
}

// Nested structures

#[test]
fn test_the_shape_example_of_the_issue() {
    let (result, output) = run(&[
        "10 DIM shape{pos{x, y}, size{w, h}}",
        "20 shape.pos.x = 3 : shape.pos.y = 4 : shape.size.w = 10",
        "30 PRINT ;shape.pos.x * shape.pos.y;\" \";shape.size.w;\" \";shape.size.h",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "12 10 0\n");
}

#[test]
fn test_a_structure_in_a_structure_2_levels_deep() {
    let (result, output) = run(&[
        "10 DIM o{name$, a{b{x, n%, t$}, y}}",
        "20 o.a.b.x = 1.5 : o.a.b.n% = 2 : o.a.b.t$ = \"deep\" : o.a.y = 9",
        "30 o.name$ = \"top\"",
        "40 PRINT ;o.a.b.x;\" \";o.a.b.n%;\" \";o.a.b.t$;\" \";o.a.y;\" \";o.name$",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "1.5 2 deep 9 top\n");
}

#[test]
fn test_an_array_member_of_a_nested_structure() {
    let (result, output) = run(&[
        "10 DIM net{layer{weights(2, 2), names$(1)}}",
        "20 FOR I% = 0 TO 2 : FOR J% = 0 TO 2 : net.layer.weights(I%, J%) = I% * 10 + J% : NEXT : NEXT",
        "30 net.layer.names$(1) = \"out\"",
        "40 PRINT ;net.layer.weights(2, 1);\" \";net.layer.names$(1)",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "21 out\n");
}

#[test]
fn test_members_with_one_name_at_different_levels() {
    let (result, output) = run(&[
        "10 DIM s{x, p{x}}",
        "20 s.x = 1 : s.p.x = 2",
        "30 PRINT ;s.x;s.p.x",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "12\n");
}

// The errors

#[test]
fn test_an_index_out_of_range() {
    for line in [
        "20 PRINT s.a(4)",
        "20 s.a(-1) = 1",
        "20 PRINT s.m(1, 3)",
        "20 PRINT s.m(1)",
        "20 PRINT s.p.b(2)",
    ] {
        let (result, _) = run(&["10 DIM s{a(3), m(2, 2), p{b(1)}}", line]);
        assert_eq!(result, Err("Subscript at line 20".to_string()), "{line}");
    }
}

#[test]
fn test_a_wrong_kind_of_member_is_bad_use_of_structure() {
    for line in [
        // An index on a member that is not an array
        "20 PRINT s.x(1)",
        "20 s.p(1) = 2",
        // A member name after a member that is not a structure
        "20 PRINT s.x.y",
        "20 s.a.y = 1",
        // An array member or a structure member with no index
        "20 PRINT s.a",
        "20 PRINT s.p",
        "20 s.p = 1",
    ] {
        let (result, _) = run(&["10 DIM s{x, a(2), p{y}}", line]);
        assert_eq!(
            result,
            Err("Bad use of structure at line 20".to_string()),
            "{line}"
        );
    }
}

#[test]
fn test_bad_use_of_structure_is_err_56() {
    let (result, output) = run(&[
        "10 ON ERROR PRINT ;ERR : END",
        "20 DIM s{x}",
        "30 PRINT s.x.y",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "56\n");
}

#[test]
fn test_a_missing_member_in_a_nested_structure() {
    let (result, _) = run(&["10 DIM s{p{x}}", "20 PRINT s.p.z"]);
    assert_eq!(
        result,
        Err("No such variable: s.p.z at line 20".to_string())
    );
    let (result, _) = run(&["10 DIM s{p{x}}", "20 s.q.x = 1"]);
    assert_eq!(result, Err("No such variable: s.q at line 20".to_string()));
}

#[test]
fn test_a_bad_member_in_a_dim() {
    let cases = [
        ("10 DIM s{a(2), a}", "Duplicate member name: a"),
        ("10 DIM s{p{x, x}}", "Duplicate member name: x"),
        ("10 DIM s{p{}}", "Expected member name in DIM"),
        ("10 DIM s{a()}", "Expected at least one expression"),
        ("10 DIM s{a(2}", "Expected ')' in DIM"),
        ("10 DIM s{p{x}", "Expected '}' in DIM"),
        (
            "10 DIM s{p{(3)x}}",
            "Arrays of structures are not supported",
        ),
        (
            "10 DIM s{p{} = q{}}",
            "A structure from a prototype is not supported",
        ),
    ];
    for (line, message) in cases {
        let (result, _) = run(&[line]);
        assert_eq!(
            result,
            Err(format!("Syntax error at line 10: {message}")),
            "{line}"
        );
    }
}

#[test]
fn test_a_negative_size_of_an_array_member() {
    let (result, _) = run(&["10 DIM s{a(-1)}"]);
    // Issue 202: a negative size gives Bad DIM, as BBC BASIC does
    assert_eq!(result, Err("Bad DIM at line 10".to_string()));
}

#[test]
fn test_an_error_in_a_dim_declares_no_structure() {
    let (result, output) = run(&[
        "10 ON ERROR PRINT \"E\" : DIM s{a(2)} : s.a(2) = 5 : PRINT ;s.a(2) : END",
        "20 DIM s{a(-1)}",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "E\n5\n");
}

/// The manual says that operations on entire arrays cannot use structure
/// members, so a whole array member is a syntax error
#[test]
fn test_a_whole_array_member_is_a_syntax_error() {
    for line in ["20 s.a() = 0", "20 PRINT SUM(s.a())", "20 PRINT s.a()"] {
        let (result, _) = run(&["10 DIM s{a(2)}", line]);
        let Err(message) = result else {
            panic!("{line} runs");
        };
        assert!(
            message.starts_with("Syntax error at line 20"),
            "{line}: {message}"
        );
    }
    let (result, _) = run(&["10 DIM s{a(2)}", "20 s.a() = 0"]);
    assert_eq!(
        result,
        Err(
            "Syntax error at line 20: A whole array member is not supported: use an index"
                .to_string()
        )
    );
}

// A member path in each place where a variable is valid

#[test]
fn test_members_with_paths_in_input() {
    let (result, output) = run_with_replies(
        &[
            "10 DIM s{a(2), p{t$}}",
            "20 I% = 2",
            "30 INPUT s.a(I%), s.p.t$",
            "40 PRINT ;s.a(2);\" \";s.p.t$",
        ],
        &["2.5,Bob"],
    );
    assert_eq!(result, Ok(()));
    assert_eq!(output, "?\n2.5 Bob\n");
}

#[test]
fn test_members_with_paths_in_input_line() {
    let (result, output) = run_with_replies(
        &[
            "10 DIM s{t$(1), p{t$}}",
            "20 INPUT LINE s.t$(1)",
            "30 INPUT LINE s.p.t$",
            "40 PRINT s.t$(1);\"/\";s.p.t$",
        ],
        &["a, b", "c"],
    );
    assert_eq!(result, Ok(()));
    assert_eq!(output, "?\n?\na, b/c\n");
}

#[test]
fn test_members_with_paths_in_read() {
    let (result, output) = run(&[
        "10 DIM s{a(2), p{n%, t$}}",
        "20 READ I%, s.a(I%), s.p.n%, s.p.t$",
        "30 PRINT ;s.a(1);\" \";s.p.n%;\" \";s.p.t$",
        "40 DATA 1, 4.5, 6, word",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "4.5 6 word\n");
}

#[test]
fn test_members_with_paths_in_swap() {
    let (result, output) = run(&[
        "10 DIM s{a(2), p{x}}",
        "20 s.a(0) = 1 : s.a(2) = 3 : s.p.x = 5",
        "30 SWAP s.a(0), s.a(2)",
        "40 SWAP s.a(0), s.p.x",
        "50 PRINT ;s.a(0);s.a(2);s.p.x",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "513\n");
}

#[test]
fn test_input_file_with_member_paths() {
    let dir = std::env::temp_dir().join(format!("bbc_issue_192_input_file_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("data.txt");
    std::fs::write(&path, "first\n2.5\n").unwrap();
    let open = format!("20 H = OPENIN \"{}\"", path.display());
    let (result, output) = run(&[
        "10 DIM s{t$(1), p{x}}",
        &open,
        "30 INPUT#H, s.t$(1), s.p.x",
        "40 CLOSE#H",
        "50 PRINT s.t$(1);\" \";s.p.x",
    ]);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "first 2.5\n");
}

#[test]
fn test_input_checks_the_member_path_before_the_reply() {
    let (result, output) = run_with_replies(&["10 DIM s{a(1)}", "20 INPUT s.a(5)"], &["1"]);
    assert_eq!(result, Err("Subscript at line 20".to_string()));
    assert_eq!(output, "");
}

#[test]
fn test_print_a_member_path_after_an_item_with_no_separator() {
    let (result, output) = run(&[
        "10 DIM s{t$(1), p{t$}}",
        "20 s.t$(1) = \"A\" : s.p.t$ = \"B\"",
        "30 PRINT \"x\"s.t$(1)s.p.t$",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "xAB\n");
}

#[test]
fn test_a_member_path_in_a_function_and_a_condition() {
    let (result, output) = run(&[
        "10 DIM s{t$(1), p{n%}}",
        "20 s.t$(1) = \"abc\" : s.p.n% = 4",
        "30 IF s.p.n% = 4 THEN PRINT ;LEN(s.t$(1));MID$(s.t$(1), s.p.n% - 2)",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "3bc\n");
}

#[test]
fn test_a_string_value_in_a_numeric_array_member_is_a_type_mismatch() {
    let (result, _) = run(&["10 DIM s{a(1)}", "20 s.a(1) = \"A\""]);
    assert_eq!(result, Err("Type mismatch at line 20".to_string()));
}

// Structure parameters (issue #194) with array members and nested structures

#[test]
fn test_a_structure_parameter_uses_the_array_and_nested_members() {
    let (result, output) = run(&[
        "10 DIM s{a(2), p{x, t$(1)}}",
        "20 PROCset(s{})",
        "30 PRINT ;s.a(2);\" \";s.p.x;\" \";s.p.t$(1)",
        "40 END",
        "50 DEF PROCset(q{})",
        "60 q.a(2) = 5 : q.p.x = q.a(2) * 2 : q.p.t$(1) = \"ok\"",
        "70 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "5 10 ok\n");
}

#[test]
fn test_a_local_structure_with_array_and_nested_members() {
    let (result, output) = run(&[
        "10 DIM s{a(1)}",
        "20 s.a(1) = 1",
        "30 PROCt",
        "40 PRINT ;s.a(1)",
        "50 END",
        "60 DEF PROCt",
        "70 LOCAL s{}",
        "80 DIM s{a(3), p{x}}",
        "90 s.a(3) = 7 : s.p.x = 8 : PRINT ;s.a(3);s.p.x",
        "100 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "78\n1\n");
}

// Old programs are unchanged

#[test]
fn test_a_variable_and_an_array_with_the_name_of_an_array_member() {
    let (result, output) = run(&[
        "10 DIM a(2), s{a(2), p{a}}",
        "20 a = 1 : a(1) = 2 : s.a(1) = 3 : s.p.a = 4",
        "30 PRINT ;a;a(1);s.a(1);s.p.a",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "1234\n");
}

// The tokens and the listing

#[test]
fn test_tokens_of_a_member_path() {
    let tokens = tokenize("shape.pos.x = 1").unwrap().tokens;
    assert_eq!(
        tokens,
        vec![
            Token::Member(
                "shape".to_string(),
                vec!["pos".to_string(), "x".to_string()]
            ),
            Token::Operator('='),
            Token::Integer(1),
        ]
    );
}

#[test]
fn test_list_shows_the_lines_as_typed() {
    for line in [
        "10 DIM layer{weights(20,20),biases(20),activation$}",
        "20 DIM shape{pos{x,y},size{w,h}}",
        "30 layer.weights(1,2) = 0.5",
        "40 PRINT shape.pos.x;net.layer.weights(I%,J%)",
    ] {
        let listed = detokenize(&tokenize(line).unwrap()).unwrap();
        assert_eq!(listed, line);
    }
}

// LVAR

#[test]
fn test_lvar_shows_array_members_and_nested_structures() {
    let output = run_binary(concat!(
        "10 DIM s{n%, a(3), m$(1, 2), p{x, q{t$}}}\n",
        "20 s.n% = 2 : s.p.x = 1.5 : s.p.q.t$ = \"z\"\n",
        "RUN\nLVAR\nEXIT\n"
    ));
    let expected = concat!(
        "s.n% = 2\n",
        "s.a(3)\n",
        "s.m$(1,2)\n",
        "s.p{}\n",
        "  s.p.x = 1.5\n",
        "  s.p.q{}\n",
        "    s.p.q.t$ = \"z\"\n",
    );
    assert!(output.contains(expected), "output: {output}");
}
