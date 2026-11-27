//! Issue 61: DATA uses the rest of its line. A colon or ELSE after DATA is
//! part of the data, and an item with no quotes is the text up to the next comma.

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::{detokenize, tokenize};

/// Store the source lines, run the program and return the result and the output.
fn run(source: &[&str]) -> (Result<(), String>, String) {
    let mut executor = Executor::new();
    let mut program = ProgramStore::new();
    for line in source {
        program.store_line(tokenize(line).expect("line tokenizes"));
    }
    let result = run_program(&mut executor, &mut program);
    (result, executor.get_output().to_string())
}

#[test]
fn test_numbers_quoted_and_unquoted_items_read_into_variables() {
    let (result, output) = run(&[
        "10 READ A%, B, C$, D$",
        "20 PRINT A%;\" \";B;\"[\";C$;\"][\";D$;\"]\"",
        "30 DATA 1, 2, \"A,B\", C D",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1 2[A,B][C D]\n");
}

#[test]
fn test_colon_in_data_is_part_of_the_item() {
    let (result, output) = run(&["10 READ A$", "20 PRINT \"[\";A$;\"]\"", "30 DATA A:B"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "[A:B]\n");
}

#[test]
fn test_colon_and_rem_after_data_are_one_text_item() {
    // BBC rule: a colon does not end DATA, so "1 : REM x" is one item
    let (result, output) = run(&[
        "10 READ A$",
        "20 RESTORE",
        "30 READ N%",
        "40 PRINT \"[\";A$;\"] \";N%",
        "50 DATA 1 : REM x",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "[1 : REM x] 1\n");
}

#[test]
fn test_unknown_character_is_kept_in_a_string_item() {
    let (result, output) = run(&[
        "10 READ A$, B$",
        "20 PRINT \"[\";A$;\"][\";B$;\"]\"",
        "30 DATA 1, ~2",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "[1][~2]\n");
}

#[test]
fn test_text_item_read_into_number_gives_leading_number() {
    let (result, output) = run(&[
        "10 READ A, B%, C%",
        "20 PRINT A;\" \";B%;\" \";C%",
        "30 DATA 3.5abc, XYZ, &FF",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "3.5 0 255\n");
}

#[test]
fn test_data_in_then_branch_before_else_does_not_stop_program() {
    let (result, output) = run(&[
        "10 A% = 1",
        "20 IF A% THEN PRINT \"y\" : DATA 1,2 ELSE PRINT \"z\"",
        "30 A% = 0",
        "40 IF A% THEN PRINT \"y\" : DATA 1,2 ELSE PRINT \"z\"",
        "50 PRINT \"done\"",
    ]);

    // The ELSE is part of the DATA text, so a false condition prints nothing
    assert_eq!(result, Ok(()));
    assert_eq!(output, "y\ndone\n");
}

#[test]
fn test_list_shows_data_line_as_typed() {
    for source in [
        "30 DATA 1, 2, \"A,B\", C D",
        "40 DATA A:B : REM x ELSE",
        "50 DATA  1,~2",
        "60 DATA",
    ] {
        let listed = detokenize(&tokenize(source).unwrap()).unwrap();
        assert_eq!(listed, source);
    }
}
