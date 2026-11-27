//! Issue 82: PRINT items with no separator between them print one after the other.

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;

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
fn test_two_numbers_with_no_separator_print_both() {
    let (result, output) = run(&["10 PRINT 10 58"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1058\n");
}

#[test]
fn test_two_strings_with_no_separator_print_both() {
    let (result, output) = run(&["10 PRINT \"A\" \"B\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "AB\n");
}

#[test]
fn test_two_string_variables_with_no_separator_print_both() {
    let (result, output) = run(&["10 A$=\"X\" : B$=\"Y\" : PRINT A$ B$"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "XY\n");
}

#[test]
fn test_subtract_with_spaces_is_one_item() {
    let (result, output) = run(&["10 PRINT 1 - 2"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "-1\n");
}

#[test]
fn test_hex_applies_to_items_until_comma() {
    // ~10 is A and ~58 is 3A; the comma moves column 3 to the zone at column 10
    let (result, output) = run(&["10 PRINT ~10 58,58"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "A3A       58\n");
}

#[test]
fn test_tab_then_string_with_no_separator() {
    let (result, output) = run(&["10 PRINT TAB(5) \"X\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "     X\n");
}
