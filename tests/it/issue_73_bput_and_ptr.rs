//! Issue #73: the BPUT# statement and the PTR# assignment.

use crate::common::run_lines as run;
use std::path::PathBuf;

/// Make an empty temporary directory for one test and return a file path in it.
fn temp_file(name: &str) -> String {
    let dir: PathBuf =
        std::env::temp_dir().join(format!("bbc_issue_73_{}_{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir is created");
    dir.join("data.bin").to_string_lossy().into_owned()
}

#[test]
fn test_bput_writes_bytes_that_bget_reads() {
    let file = temp_file("bput_bget");
    let open_out = format!("10 F% = OPENOUT(\"{}\")", file);
    let open_in = format!("50 G% = OPENIN(\"{}\")", file);
    // 321 has the low 8 bits 65, so BPUT# writes "A" (D1).
    let (result, output) = run(&[
        &open_out,
        "20 BPUT#F%, 72",
        "30 BPUT#F%, 105 : BPUT#F%, 321",
        "40 CLOSE#F%",
        &open_in,
        "60 A% = BGET#G% : B% = BGET#G% : C% = BGET#G%",
        "70 PRINT A%; \" \"; B%; \" \"; C%; \" \"; EOF#G%",
        "80 CLOSE#G%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "        72 105 65 -1\n");
    assert_eq!(std::fs::read(&file).expect("file exists"), b"HiA");
}

#[test]
fn test_ptr_assignment_moves_to_second_byte() {
    let file = temp_file("ptr_set");
    let open_out = format!("10 F% = OPENOUT(\"{}\")", file);
    let open_in = format!("40 G% = OPENIN(\"{}\")", file);
    let (result, output) = run(&[
        &open_out,
        "20 BPUT#F%, 65 : BPUT#F%, 66 : BPUT#F%, 67",
        "30 CLOSE#F%",
        &open_in,
        "50 PTR#G% = 1",
        "60 PRINT PTR#G%; \" \"; BGET#G%",
        "70 CLOSE#G%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         1 66\n");
}

#[test]
fn test_bput_and_ptr_in_if_branch() {
    let file = temp_file("if_branch");
    let open_out = format!("10 F% = OPENOUT(\"{}\")", file);
    let open_in = format!("40 G% = OPENIN(\"{}\")", file);
    let (result, output) = run(&[
        &open_out,
        "20 IF 1 = 1 THEN BPUT#F%, 88 : BPUT#F%, 89",
        "30 CLOSE#F%",
        &open_in,
        "50 IF 1 = 1 THEN PTR#G% = 1",
        "60 PRINT BGET#G%",
        "70 CLOSE#G%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "        89\n");
}

#[test]
fn test_negative_ptr_gives_error() {
    let file = temp_file("ptr_negative");
    let open_out = format!("10 F% = OPENOUT(\"{}\")", file);
    let (result, _) = run(&[&open_out, "20 PTR#F% = -1"]);

    let message = result.expect_err("a negative position gives an error");
    assert_eq!(message, "Illegal function at line 20");
}

#[test]
fn test_bput_with_closed_channel_gives_channel_error() {
    let (result, _) = run(&["10 BPUT#99, 65"]);

    let message = result.expect_err("a closed channel gives an error");
    assert_eq!(message, "Channel at line 10");
}

#[test]
fn test_ptr_assignment_with_closed_channel_gives_channel_error() {
    let (result, _) = run(&["10 PTR#99 = 0"]);

    let message = result.expect_err("a closed channel gives an error");
    assert_eq!(message, "Channel at line 10");
}
