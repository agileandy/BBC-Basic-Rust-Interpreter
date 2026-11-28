//! Issue #193: arrays of structures, a structure from a prototype, and the
//! copy of a whole structure, as in BBC BASIC for Windows (manual page
//! bbcwin2.html#structures): `DIM t{(1000) f(10), n%}`, `t{(I%)}.n% = 1`,
//! `DIM b{} = a{}` and `b{} = a{}`.

use crate::common::run_binary_read_first as run_binary;
use crate::common::run_with_limit as run;
use crate::common::run_with_replies;
use bbc_basic_interpreter::tokenizer::{detokenize, tokenize, Token};

// Arrays of structures

#[test]
fn test_an_array_of_1000_structures_in_a_loop() {
    let (result, output) = run(&[
        "10 DIM training{(1000) features(10), label%, name$}",
        "20 FOR I% = 0 TO 1000",
        "30   training{(I%)}.label% = I% MOD 2",
        "40   training{(I%)}.features(3) = I% / 4",
        "50   training{(I%)}.name$ = \"p\" + STR$ I%",
        "60 NEXT",
        "70 S% = 0 : FOR I% = 0 TO 1000 : S% = S% + training{(I%)}.label% : NEXT",
        "80 PRINT ;S%;\" \";training{(999)}.name$;\" \";training{(1000)}.features(3);\" \";training{(1)}.features(2)",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "500 p999 250 0\n");
}

#[test]
fn test_an_array_of_structures_with_2_dimensions() {
    let (result, output) = run(&[
        "10 DIM g{(2, 3) n%, s$}",
        "20 g{(1, 2)}.n% = 5 : g{(2, 3)}.s$ = \"end\"",
        "30 PRINT ;g{(1, 2)}.n%;\" \";g{(2, 3)}.s$;\" \";g{(2, 2)}.n%",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "5 end 0\n");
}

#[test]
fn test_an_array_of_structures_with_a_sub_structure() {
    let (result, output) = run(&[
        "10 DIM s{(3) pos{x, y}, v%(2)}",
        "20 I% = 2 : s{(I%)}.pos.x = 1.5 : s{(I% + 1)}.v%(2) = 7",
        "30 PRINT ;s{(2)}.pos.x;\" \";s{(3)}.v%(2);\" \";s{(3)}.pos.x",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "1.5 7 0\n");
}

#[test]
fn test_an_index_out_of_range_gives_subscript() {
    for line in [
        "20 g{(3)}.n% = 1",
        "20 PRINT g{(-1)}.n%",
        "20 PRINT g{(1, 1)}.n%",
    ] {
        let (result, _) = run(&["10 DIM g{(2) n%}", line]);
        assert_eq!(result, Err("Subscript at line 20".to_string()), "{line}");
    }
}

#[test]
fn test_a_wrong_use_of_an_index_is_bad_use_of_structure() {
    for line in [
        "20 p{(1)}.x = 1",
        "20 PRINT p{(1)}.x",
        "20 t.x = 1",
        "20 PRINT t.x",
        "20 PROCa(p{(0)})",
    ] {
        let (result, _) = run(&[
            "10 DIM p{x}, t{(3) x}",
            line,
            "30 END",
            "40 DEF PROCa(s{})",
            "50 ENDPROC",
        ]);
        assert_eq!(
            result,
            Err("Bad use of structure at line 20".to_string()),
            "{line}"
        );
    }
}

#[test]
fn test_a_second_dim_of_an_array_of_structures_is_bad_dim() {
    let (result, _) = run(&["10 DIM t{(3) x}", "20 DIM t{(3) x}"]);
    assert_eq!(result, Err("Bad DIM at line 20".to_string()));
}

#[test]
fn test_a_bad_array_of_structures_is_a_syntax_error() {
    let cases = [
        ("10 DIM t{(3)}", "Expected member name in DIM"),
        ("10 DIM t{(3) x", "Expected '}' in DIM"),
        ("10 DIM t{(3 x}", "Expected ')' in DIM"),
        (
            "10 DIM s{p{(3)x}}",
            "Arrays of structures are not supported",
        ),
        ("10 PRINT t{(1).x", "Expected '}' after the structure index"),
        (
            "10 PRINT t{()}.x",
            "A whole array of structures is not supported",
        ),
        ("10 DIM b{} = 5", "Expected a structure after '='"),
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
fn test_members_of_elements_in_input_read_and_swap() {
    let (result, output) = run_with_replies(
        &[
            "10 DIM t{(2) x, s$, a(1)}",
            "20 INPUT t{(1)}.x, t{(2)}.s$",
            "30 INPUT LINE t{(0)}.s$",
            "40 READ t{(2)}.x, t{(0)}.a(1)",
            "50 SWAP t{(1)}.x, t{(2)}.x",
            "60 PRINT ;t{(1)}.x;\" \";t{(2)}.x;\" \";t{(2)}.s$;\" \";t{(0)}.s$;\" \";t{(0)}.a(1)",
            "70 DATA 9, 4",
        ],
        &["2.5,Bob", "a, b"],
    );
    assert_eq!(result, Ok(()));
    assert_eq!(output, "?\n?\n9 2.5 Bob a, b 4\n");
}

#[test]
fn test_input_file_with_members_of_elements() {
    let dir = std::env::temp_dir().join(format!("bbc_issue_193_input_file_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("data.txt");
    std::fs::write(&path, "first\n2.5\n").unwrap();
    let open = format!("20 H = OPENIN \"{}\"", path.display());
    let (result, output) = run(&[
        "10 DIM t{(1) s$, x}",
        &open,
        "30 INPUT#H, t{(1)}.s$, t{(0)}.x",
        "40 CLOSE#H",
        "50 PRINT t{(1)}.s$;\" \";t{(0)}.x",
    ]);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "first 2.5\n");
}

#[test]
fn test_an_element_is_an_argument_by_reference() {
    let (result, output) = run(&[
        "10 DIM t{(3) x, n%}",
        "20 t{(3)}.x = 2",
        "30 PROCmove(t{(3)}, 1)",
        "40 PRINT ;t{(3)}.x;\" \";t{(3)}.n%;\" \";t{(2)}.x;\" \";FNsum(t{(3)})",
        "50 END",
        "60 DEF PROCmove(p{}, dx)",
        "70 p.x = p.x + dx : p.n% = 10",
        "80 ENDPROC",
        "90 DEF FNsum(p{}) = p.x + p.n%",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "3 10 0 13\n");
}

#[test]
fn test_a_variable_and_an_array_with_the_name_of_an_array_of_structures() {
    let (result, output) = run(&[
        "10 DIM t{(2) x}, t(2)",
        "20 t = 5 : t(1) = 6 : t{(1)}.x = 7",
        "30 PRINT ;t;\" \";t(1);\" \";t{(1)}.x",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "5 6 7\n");
}

// Structures from a prototype

#[test]
fn test_a_structure_from_a_prototype_has_zero_values() {
    let (result, output) = run(&[
        "10 DIM a{x, n%, s$}",
        "20 a.x = 5 : a.n% = 2 : a.s$ = \"a\"",
        "30 DIM b{} = a{}",
        "40 PRINT ;b.x;\" \";b.n%;\"/\";b.s$;\"/\"",
        "50 b.x = 1 : b.s$ = \"b\"",
        "60 PRINT ;b.x;\" \";b.s$;\" \";a.x;\" \";a.s$",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "0 0//\n1 b 5 a\n");
}

#[test]
fn test_a_prototype_with_nested_and_array_members() {
    let (result, output) = run(&[
        "10 DIM a{p{x, y}, v%(3), t$(1)}",
        "20 a.v%(2) = 9 : a.p.y = 4",
        "30 DIM b{} = a{}",
        "40 b.p.y = 2 : b.v%(3) = 4 : b.t$(1) = \"z\"",
        "50 PRINT ;b.p.y;\" \";b.v%(3);\" \";b.v%(2);\" \";b.t$(1);\" \";a.p.y;\" \";a.v%(3)",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "2 4 0 z 4 0\n");
}

#[test]
fn test_an_array_of_structures_from_a_prototype() {
    let (result, output) = run(&[
        "10 DIM a{x, n%}",
        "20 DIM c{(10)} = a{}, d{(1, 2)} = a{}",
        "30 c{(10)}.x = 3 : d{(1, 2)}.n% = 6",
        "40 PRINT ;c{(10)}.x;\" \";d{(1, 2)}.n%;\" \";c{(0)}.x",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "3 6 0\n");
}

#[test]
fn test_a_sub_structure_from_a_prototype() {
    let (result, output) = run(&[
        "10 DIM a{x, y}",
        "20 DIM n{name$, pos{} = a{}}",
        "30 n.pos.y = 2 : n.name$ = \"n\"",
        "40 PRINT n.name$;\" \";n.pos.y;\" \";n.pos.x",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "n 2 0\n");
}

#[test]
fn test_an_element_is_a_prototype() {
    let (result, output) = run(&[
        "10 DIM t{(2) x, s$}",
        "20 DIM b{} = t{(1)}",
        "30 b.s$ = \"b\" : PRINT b.s$;\" \";b.x",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "b 0\n");
}

#[test]
fn test_a_local_structure_is_not_a_prototype() {
    // The manual, section "Structure prototypes": "You cannot use a LOCAL
    // structure as a prototype". BBC BASIC for SDL gives Bad DIM.
    let (result, _) = run(&[
        "10 PROCa",
        "20 END",
        "30 DEF PROCa",
        "40 LOCAL s{}",
        "50 DIM s{x}",
        "60 DIM b{} = s{}",
        "70 ENDPROC",
    ]);
    assert_eq!(result, Err("Bad DIM at line 60".to_string()));
}

#[test]
fn test_a_prototype_that_is_not_a_structure() {
    let (result, _) = run(&["10 DIM b{} = a{}"]);
    assert_eq!(result, Err("Bad use of structure at line 10".to_string()));
}

// The copy of a structure

#[test]
fn test_a_copy_is_deep() {
    let (result, output) = run(&[
        "10 DIM a{x, n%, s$, v(2), p{y}}",
        "20 a.x = 1 : a.n% = 2 : a.s$ = \"a\" : a.v(1) = 3 : a.p.y = 4",
        "30 DIM b{} = a{}",
        "40 b{} = a{}",
        "50 PRINT ;b.x;\" \";b.n%;\" \";b.s$;\" \";b.v(1);\" \";b.p.y",
        "60 b.x = 9 : b.s$ = \"b\" : b.v(1) = 8 : b.p.y = 7",
        "70 PRINT ;a.x;\" \";a.s$;\" \";a.v(1);\" \";a.p.y",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "1 2 a 3 4\n1 a 3 4\n");
}

#[test]
fn test_a_copy_to_and_from_an_element() {
    let (result, output) = run(&[
        "10 DIM a{x, s$}, t{(2) x, s$}",
        "20 a.x = 1 : a.s$ = \"a\"",
        "30 t{(2)} = a{} : a.x = 5",
        "40 t{(1)} = t{(2)} : t{(2)}.x = 6",
        "50 IF 1 THEN a{} = t{(1)}",
        "60 PRINT ;a.x;\" \";a.s$;\" \";t{(1)}.x;\" \";t{(2)}.x",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "1 a 1 6\n");
}

#[test]
fn test_a_copy_of_2_structures_that_do_not_match_is_a_type_mismatch() {
    // BBC BASIC for SDL compares the sizes and gives Type mismatch. This
    // interpreter compares the type of each member, in the order of the DIM.
    for dim in [
        "10 DIM a{x}, b{x, y}",
        "10 DIM a{x}, b{x%}",
        "10 DIM a{v(2)}, b{v(3)}",
        "10 DIM a{p{x}}, b{p{x%}}",
        "10 DIM a{s$}, b{p{s$}}",
    ] {
        let (result, _) = run(&[dim, "20 b{} = a{}"]);
        assert_eq!(result, Err("Type mismatch at line 20".to_string()), "{dim}");
    }
}

#[test]
fn test_a_copy_with_other_member_names_copies_by_position() {
    let (result, output) = run(&[
        "10 DIM a{x, s$}, b{y, t$}",
        "20 a.x = 3 : a.s$ = \"a\"",
        "30 b{} = a{}",
        "40 PRINT ;b.y;\" \";b.t$",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "3 a\n");
}

#[test]
fn test_a_copy_of_a_structure_that_does_not_exist() {
    let (result, _) = run(&["10 DIM a{x}", "20 b{} = a{}"]);
    assert_eq!(result, Err("Bad use of structure at line 20".to_string()));
    let (result, _) = run(&["10 DIM a{x}", "20 a{} = c{}"]);
    assert_eq!(result, Err("Bad use of structure at line 20".to_string()));
}

#[test]
fn test_a_whole_structure_in_an_expression_is_still_a_syntax_error() {
    for line in ["20 PRINT a{}", "20 x = a{}", "20 b{} = a{} + 1"] {
        let (result, _) = run(&["10 DIM a{x}, b{x}", line]);
        assert!(
            matches!(result, Err(ref message) if message.starts_with("Syntax error at line 20")),
            "{line}: {result:?}"
        );
    }
}

#[test]
fn test_a_whole_structure_beside_an_equals_is_a_copy_only_at_the_start() {
    let whole = "Syntax error at line 20: A reference to a whole structure is not supported";
    for line in [
        "20 PRINT a{} = b{}",
        "20 IF a{} = b{} THEN PRINT 1",
        "20 x = 1 + a{} = b{}",
        "20 PRINT 1 : PRINT a{} = b{}",
    ] {
        let (result, _) = run(&["10 DIM a{x}, b{x}", line]);
        assert_eq!(result, Err(whole.to_string()), "{line}");
    }
    let (result, output) = run(&[
        "10 DIM a{x}, b{x} : a.x = 4",
        "20 PRINT ;1; : b{} = a{} : IF 1 THEN PRINT ;b.x ELSE b{} = a{}",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "14\n");
}

#[test]
fn test_an_element_with_no_member_is_only_an_argument_or_a_copy() {
    let whole = "Syntax error at line 20: A reference to a whole structure is not supported";
    for line in [
        "20 PRINT t{(1)}",
        "20 x = t{(1)}",
        "20 READ t{(1)}",
        "20 SWAP t{(1)}, t{(2)}",
        "20 INPUT t{(1)}",
        "20 PRINT FNa(t{(1)} + 1)",
        "20 IF t{(1)} = a{} THEN PRINT 1",
    ] {
        let (result, _) = run(&["10 DIM t{(2) x}, a{x}", line]);
        assert_eq!(result, Err(whole.to_string()), "{line}");
    }
}

// LOCAL and the DIM space

#[test]
fn test_an_array_of_structures_for_a_local_structure_is_bad_dim() {
    // The manual names no LOCAL form for an array of structures in this
    // interpreter: LOCAL t{} declares a structure, so its DIM must be a
    // structure.
    for dim in ["50 DIM t{(2) x}", "50 DIM t{(2)} = a{}"] {
        let (result, _) = run(&[
            "10 DIM a{x}",
            "20 PROCa",
            "30 END",
            "40 DEF PROCa : LOCAL t{}",
            dim,
            "60 ENDPROC",
        ]);
        assert_eq!(result, Err("Bad DIM at line 50".to_string()), "{dim}");
    }
}

#[test]
fn test_a_dim_that_is_too_large_gives_no_room() {
    for dim in [
        "10 DIM t{(100000000) x, s$}",
        "10 DIM t{(3) a(100000000)}",
        "10 DIM s{a$(100000000)}",
        "10 DIM a(100000000)",
        "10 DIM a$(30000000)",
        "10 DIM a%(100000, 100000, 100000)",
        "10 DIM a{x} : DIM t{(100000000)} = a{}",
    ] {
        let (result, _) = run(&[dim]);
        assert_eq!(result, Err("No room at line 10".to_string()), "{dim}");
    }
    let (result, output) = run(&["10 DIM a(1000000), t{(10000) x}", "20 PRINT ;a(1000000)"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "0\n");
}

// Tokens, LIST and LVAR

#[test]
fn test_tokens_of_a_member_of_an_element() {
    let tokens = tokenize("t{(I%)}.p.x = 1").unwrap().tokens;
    assert_eq!(
        tokens,
        vec![
            Token::Identifier("t".to_string()),
            Token::Separator('{'),
            Token::Separator('('),
            Token::Identifier("I%".to_string()),
            Token::Separator(')'),
            Token::Separator('}'),
            Token::Member(String::new(), vec!["p".to_string(), "x".to_string()]),
            Token::Operator('='),
            Token::Integer(1),
        ]
    );
}

#[test]
fn test_list_shows_the_lines_as_typed() {
    for line in [
        "10 DIM training{(1000) features(10),label%,name$}",
        "20 training{(I%)}.label% = 1",
        "30 PRINT g{(1,2)}.p.x;t{(1)}.a(2)",
        "40 DIM b{} = a{},c{(10)} = a{},n{name$,pos{} = a{}}",
        "50 b{} = a{}",
        "60 PROCmove(t{(3)},1)",
        "70 IF 1 THEN b{} = a{} ELSE a{} = b{}",
        "80 DIM t{(2) x,y},g{(N%,2) p{x}}",
        "90 IF 1 THEN DIM p{x} ELSE PRINT a(1)",
    ] {
        let listed = detokenize(&tokenize(line).unwrap()).unwrap();
        assert_eq!(listed, line);
    }
}

#[test]
fn test_lvar_shows_an_array_of_structures_as_one_line() {
    let output = run_binary(concat!(
        "10 DIM t{(1000) f(10), n%, p{x, s$}}, g{(2, 3) a}\n",
        "20 t{(5)}.n% = 2\n",
        "RUN\nLVAR\nEXIT\n"
    ));
    let expected = "g{(2,3) a}\nt{(1000) f(10),n%,p{x,s$}}\n";
    assert!(output.contains(expected), "output: {output}");
    assert!(!output.contains("t.n%"), "output: {output}");
}

#[test]
fn test_the_real_binary_runs_an_array_of_structures() {
    let output = run_binary(concat!(
        "10 DIM pt{x, y}\n",
        "20 DIM path{(2)} = pt{}\n",
        "30 FOR I% = 0 TO 2 : path{(I%)}.x = I% * 10 : NEXT\n",
        "40 DIM last{} = pt{} : last{} = path{(2)}\n",
        "50 PRINT ;path{(1)}.x;\" \";last.x\n",
        "RUN\nEXIT\n"
    ));
    assert!(output.contains("10 20\n"), "output: {output}");
}

// A member of an element as the FOR variable (issue #198)

#[test]
fn test_a_member_of_an_element_is_a_for_variable() {
    let (result, output) = run(&[
        "10 DIM t{(2) x, n%, a(1)}",
        "20 FOR t{(1)}.x = 1 TO 3 : PRINT ;t{(1)}.x; : NEXT",
        "30 I% = 2 : FOR t{(I%)}.n% = 5 TO 6 : NEXT t{(I%)}.n%",
        "40 FOR t{(0)}.a(1) = 1 TO 2 STEP 0.5 : NEXT",
        "50 PRINT \" \";t{(1)}.x;\" \";t{(2)}.n%;\" \";t{(0)}.a(1);\" \";t{(0)}.x",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "123 4 7 2.5 0\n");
}
