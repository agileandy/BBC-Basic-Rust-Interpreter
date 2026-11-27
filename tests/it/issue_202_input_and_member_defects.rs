//! Issue 202: 4 small defects of INPUT targets and structure members.
//!
//! 1. A token after an INPUT item that INPUT cannot use gives a syntax error.
//! 2. A member prints in the number format of `@%`, as a variable does.
//! 3. INPUT, INPUT LINE, INPUT# and READ evaluate the indices of a target
//!    one time.
//! 4. A negative size in DIM gives Bad DIM (`ERR` 10), also for a member.
//!
//! The reference is the manual of BBC BASIC for Windows, page
//! bbcwin2.html#structures.

use crate::common::{run_lines as run, run_with_replies};
use bbc_basic_interpreter::tokenizer::tokenize;

/// Run the program with the replies and check that it gives the error and
/// prints nothing.
fn check_input_error(source: &[&str], message: &str) {
    let (result, output) = run_with_replies(source, &["5"]);
    assert_eq!(result, Err(message.to_string()), "program: {:?}", source);
    assert_eq!(output, "", "program: {:?}", source);
}

#[test]
fn test_hash_after_an_input_variable() {
    check_input_error(
        &["10 INPUT A #", "20 PRINT \"RAN\""],
        "Syntax error at line 10: Unexpected # in INPUT",
    );
}

/// The tokenizer gives the error for `]` before the parser sees it (issue 62)
#[test]
fn test_close_square_bracket_after_a_prompt_and_a_variable() {
    let error = tokenize("10 INPUT \"x\" A ]").unwrap_err();
    assert_eq!(
        error.to_string(),
        "Syntax error at line 10: Unknown character ']'"
    );
}

#[test]
fn test_stray_operator_after_a_prompt_and_a_variable() {
    check_input_error(
        &["10 INPUT \"x\" A =", "20 PRINT \"RAN\""],
        "Syntax error at line 10: Unexpected = in INPUT",
    );
}

#[test]
fn test_stray_token_after_an_input_line_variable() {
    check_input_error(
        &["10 INPUT LINE A$ #", "20 PRINT \"RAN\""],
        "Syntax error at line 10: Unexpected # in INPUT",
    );
}

#[test]
fn test_stray_token_after_an_input_member() {
    check_input_error(
        &["10 DIM p{x}", "20 INPUT p.x #", "30 PRINT \"RAN\""],
        "Syntax error at line 20: Unexpected # in INPUT",
    );
}

#[test]
fn test_valid_input_forms_are_unchanged() {
    let cases: [(&[&str], &[&str], &str); 6] = [
        (&["10 INPUT A", "20 PRINT ;A"], &["5"], "?\n5\n"),
        (
            &["10 INPUT \"N\" A, B", "20 PRINT ;A;B"],
            &["1,2"],
            "N\n12\n",
        ),
        (
            &["10 INPUT \"N\", A; B$", "20 PRINT ;A;B$"],
            &["1,X"],
            "N?\n1X\n",
        ),
        (&["10 INPUT ' A", "20 PRINT ;A"], &["3"], "\n?\n3\n"),
        (
            &["10 DIM A(2)", "20 INPUT A(1)", "30 PRINT ;A(1)"],
            &["7"],
            "?\n7\n",
        ),
        (
            &["10 DIM p{x}", "20 INPUT p.x", "30 PRINT ;p.x"],
            &["4"],
            "?\n4\n",
        ),
    ];
    for (source, replies, expected) in cases {
        let (result, output) = run_with_replies(source, replies);
        assert_eq!(result, Ok(()), "program: {:?}", source);
        assert_eq!(output, expected, "program: {:?}", source);
    }
}

#[test]
fn test_an_integer_member_uses_the_default_format() {
    let (result, output) = run(&["10 DIM p{y%}", "20 p.y% = 1234567890", "30 PRINT ;p.y%"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "1.23456789E9\n");
}

#[test]
fn test_a_real_member_uses_the_format_of_print_format_variable() {
    let (result, output) = run(&[
        "10 DIM p{x}",
        "20 p.x = 0.05",
        "30 @% = &20209",
        "40 PRINT ;p.x",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "0.05\n");
}

/// A member and a variable with the same value print the same text, for 4
/// values and 2 formats
#[test]
fn test_a_member_prints_as_a_variable_prints() {
    for format in ["&90A", "&20209"] {
        for (value, member, variable) in [
            ("1234567890", "p.i%", "i%"),
            ("-7", "p.i%", "i%"),
            ("0.05", "p.r", "r"),
            ("1E10/3", "p.r", "r"),
        ] {
            let (result, output) = run(&[
                "10 DIM p{i%, r}",
                &format!("20 {member} = {value} : {variable} = {value}"),
                &format!("30 @% = {format}"),
                &format!("40 PRINT {member} : PRINT {variable}"),
            ]);
            assert_eq!(result, Ok(()), "{format} {value}");
            let lines: Vec<&str> = output.lines().collect();
            assert_eq!(lines.len(), 2, "{format} {value}: {output:?}");
            assert_eq!(lines[0], lines[1], "{format} {value}");
        }
    }
}

/// The lines of `FNi`: a function that counts its calls in C% and gives 1
const COUNTER_FUNCTION: [&str; 3] = ["90 DEF FNi", "91 C% = C% + 1", "92 = 1"];

/// Run the program with the counter function and a reply of 4, and give the
/// output
fn run_with_counter(source: &[&str]) -> String {
    let lines: Vec<&str> = source.iter().chain(&COUNTER_FUNCTION).copied().collect();
    let (result, output) = run_with_replies(&lines, &["4"]);
    assert_eq!(result, Ok(()), "program: {:?}", source);
    output
}

#[test]
fn test_input_evaluates_the_index_of_a_member_one_time() {
    let output = run_with_counter(&[
        "10 DIM s{a(2)}",
        "20 INPUT s.a(FNi)",
        "30 PRINT ;C%;\" \";s.a(1)",
        "40 END",
    ]);
    assert_eq!(output, "?\n1 4\n");
}

#[test]
fn test_input_line_evaluates_the_index_of_a_member_one_time() {
    let output = run_with_counter(&[
        "10 DIM s{a$(2)}",
        "20 INPUT LINE s.a$(FNi)",
        "30 PRINT ;C%;\" \";s.a$(1)",
        "40 END",
    ]);
    assert_eq!(output, "?\n1 4\n");
}

#[test]
fn test_read_evaluates_the_index_of_a_member_one_time() {
    let output = run_with_counter(&[
        "10 DIM s{a(2)}",
        "20 READ s.a(FNi)",
        "30 PRINT ;C%;\" \";s.a(1)",
        "40 END",
        "50 DATA 6",
    ]);
    assert_eq!(output, "1 6\n");
}

#[test]
fn test_input_evaluates_the_index_of_an_array_element_one_time() {
    let output = run_with_counter(&[
        "10 DIM A(2)",
        "20 INPUT A(FNi)",
        "30 PRINT ;C%;\" \";A(1)",
        "40 END",
    ]);
    assert_eq!(output, "?\n1 4\n");
}

#[test]
fn test_read_evaluates_the_index_of_an_array_element_one_time() {
    let output = run_with_counter(&[
        "10 DIM A(2)",
        "20 READ A(FNi)",
        "30 PRINT ;C%;\" \";A(1)",
        "40 END",
        "50 DATA 6",
    ]);
    assert_eq!(output, "1 6\n");
}

#[test]
fn test_input_file_evaluates_the_index_of_a_target_one_time() {
    for (dim, target) in [("s{a(2)}", "s.a(FNi)"), ("A(2)", "A(FNi)")] {
        let file = std::env::temp_dir().join(format!(
            "bbc_issue_202_{}_{}",
            dim.len(),
            std::process::id()
        ));
        let file = file.to_string_lossy().replace('\\', "/");
        let output = run_with_counter(&[
            &format!("10 DIM {dim}"),
            &format!("20 H = OPENOUT(\"{file}\") : PRINT# H, 8 : CLOSE# H"),
            &format!("30 H = OPENIN(\"{file}\") : INPUT# H, {target} : CLOSE# H"),
            &format!("40 PRINT ;C%;\" \";{}", target.replace("FNi", "1")),
            "50 END",
        ]);
        let _ = std::fs::remove_file(&file);
        assert_eq!(output, "1 8\n", "target: {target}");
    }
}

#[test]
fn test_a_negative_dim_size_gives_bad_dim() {
    for dim in ["A(-1)", "A%(2, -1)", "s{a(-1)}", "s{x, b$(-2)}"] {
        let (result, _) = run(&[&format!("10 DIM {dim}")]);
        assert_eq!(result, Err("Bad DIM at line 10".to_string()), "DIM {dim}");
    }
}

#[test]
fn test_bad_dim_has_err_10() {
    for dim in ["A(-1)", "s{a(-1)}"] {
        let (result, output) = run(&[
            "10 ON ERROR PRINT ;ERR;\" \";REPORT$ : END",
            &format!("20 DIM {dim}"),
        ]);
        assert_eq!(result, Ok(()), "DIM {dim}");
        assert_eq!(output, "10 Bad DIM\n", "DIM {dim}");
    }
}
