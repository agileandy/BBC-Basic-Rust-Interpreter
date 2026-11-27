//! Issue #90: PRINT# reads its items with the same reader as PRINT.

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;
use std::path::PathBuf;

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

/// Make an empty temporary directory for one test and return a file path in it.
fn temp_file(name: &str) -> String {
    let dir: PathBuf =
        std::env::temp_dir().join(format!("bbc_issue_90_{}_{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir is created");
    dir.join("data.txt").to_string_lossy().into_owned()
}

/// Run a program that opens the file, runs one PRINT# line and closes the file.
/// Return the bytes of the file.
fn write_with(name: &str, print_line: &str) -> Vec<u8> {
    let file = temp_file(name);
    let open_out = format!("10 F% = OPENOUT(\"{}\")", file);
    let print = format!("20 {}", print_line);
    let (result, _) = run(&[&open_out, &print, "30 CLOSE#F%"]);
    assert_eq!(result, Ok(()));
    std::fs::read(&file).expect("file is readable")
}

#[test]
fn test_print_file_two_numbers_with_no_separator_writes_both() {
    let bytes = write_with("two_numbers", "PRINT#F%, 10 58");

    assert_eq!(String::from_utf8_lossy(&bytes), "1058\n");
}

#[test]
fn test_print_file_spc_parses_and_writes_no_spaces() {
    // D4: the executor ignores SPC in PRINT#, as it did before this issue.
    let bytes = write_with("spc", "PRINT#F%, SPC(3);\"A\"");

    assert_eq!(String::from_utf8_lossy(&bytes), "A\n");
}

#[test]
fn test_print_file_tan_is_a_function() {
    let bytes = write_with("tan", "PRINT#F%, TAN(0)");

    assert_eq!(String::from_utf8_lossy(&bytes), "0\n");
}

#[test]
fn test_print_file_hex_form() {
    let bytes = write_with("hex", "PRINT#F%, ~255;255");

    assert_eq!(String::from_utf8_lossy(&bytes), "FF255\n");
}

#[test]
fn test_print_file_number_comma_string() {
    // D3: this build writes text. A comma writes a tab character.
    let bytes = write_with("comma", "PRINT#F%, 1, \"A\"");

    assert_eq!(String::from_utf8_lossy(&bytes), "1\tA\n");
}
