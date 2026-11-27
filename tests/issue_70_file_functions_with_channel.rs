//! Issue #70: BGET#, EOF#, PTR# and EXT# work with a file channel in an expression.

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
        std::env::temp_dir().join(format!("bbc_issue_70_{}_{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir is created");
    dir.join("data.txt").to_string_lossy().into_owned()
}

#[test]
fn test_bget_reads_bytes_until_eof() {
    let file = temp_file("bget_eof");
    let open_out = format!("10 F% = OPENOUT(\"{}\")", file);
    let open_in = format!("40 G% = OPENIN(\"{}\")", file);
    // PRINT# writes "ABC" and a newline: 4 bytes with the sum 65+66+67+10 = 208.
    // The loop stops after 100 bytes if EOF# never becomes true.
    let (result, output) = run(&[
        &open_out,
        "20 PRINT#F%, \"ABC\"",
        "30 CLOSE#F%",
        &open_in,
        "50 N% = 0 : S% = 0",
        "60 REPEAT",
        "70 S% = S% + BGET#G%",
        "80 N% = N% + 1",
        "90 UNTIL EOF#G% OR N% > 100",
        "100 CLOSE#G%",
        "110 PRINT N%; \" \"; S%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "4 208\n");
}

#[test]
fn test_ptr_and_ext_with_channel() {
    let file = temp_file("ptr_ext");
    let open_out = format!("10 F% = OPENOUT(\"{}\")", file);
    let open_in = format!("40 G% = OPENIN(\"{}\")", file);
    let (result, output) = run(&[
        &open_out,
        "20 PRINT#F%, \"ABC\"",
        "30 CLOSE#F%",
        &open_in,
        "50 B% = BGET#G%",
        "60 PRINT PTR#G%; \" \"; EXT#G%",
        "70 CLOSE#G%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1 4\n");
}

#[test]
fn test_eof_with_brackets_still_works() {
    let file = temp_file("eof_brackets");
    let open_out = format!("10 F% = OPENOUT(\"{}\")", file);
    let open_in = format!("40 G% = OPENIN(\"{}\")", file);
    let (result, output) = run(&[
        &open_out,
        "20 PRINT#F%, \"A\"",
        "30 CLOSE#F%",
        &open_in,
        "50 PRINT EOF(G%)",
        "60 CLOSE#G%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0\n");
}

#[test]
fn test_bget_with_closed_channel_gives_channel_error() {
    let (result, _) = run(&["10 X% = BGET#99"]);

    let message = result.expect_err("a closed channel gives an error");
    assert_eq!(message, "Channel at line 10");
}

#[test]
fn test_eof_with_closed_channel_gives_channel_error() {
    let (result, _) = run(&["10 X% = EOF#99"]);

    let message = result.expect_err("a closed channel gives an error");
    assert_eq!(message, "Channel at line 10");
}
