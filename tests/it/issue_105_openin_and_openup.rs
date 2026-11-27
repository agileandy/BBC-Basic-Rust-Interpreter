//! Issue #105: OPENIN gives 0 for a file that is not there, and OPENUP opens
//! a file for read and write.

use crate::common::run_lines as run;
use std::path::PathBuf;

/// Make an empty temporary directory for one test and return a file path in it.
fn temp_file(name: &str) -> String {
    let dir: PathBuf =
        std::env::temp_dir().join(format!("bbc_issue_105_{}_{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir is created");
    dir.join("data.bin").to_string_lossy().into_owned()
}

#[test]
fn test_openin_of_missing_file_gives_zero_and_program_continues() {
    let file = temp_file("openin_missing");
    let open_in = format!("10 F% = OPENIN(\"{}\")", file);
    let (result, output) = run(&[&open_in, "20 PRINT F%", "30 PRINT \"MORE\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         0\nMORE\n");
}

#[test]
fn test_openin_of_existing_file_gives_handle_that_is_not_zero() {
    let file = temp_file("openin_exists");
    std::fs::write(&file, b"ABC").expect("file is written");
    let open_in = format!("10 F% = OPENIN(\"{}\")", file);
    let (result, output) = run(&[&open_in, "20 PRINT F% <> 0", "30 CLOSE#F%"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "        -1\n");
}

#[test]
fn test_openup_changes_one_byte_and_keeps_the_length() {
    let file = temp_file("openup_change");
    std::fs::write(&file, b"ABC").expect("file is written");
    let open_up = format!("10 F% = OPENUP(\"{}\")", file);
    let open_in = format!("60 G% = OPENIN(\"{}\")", file);
    let (result, output) = run(&[
        &open_up,
        "20 PRINT PTR#F%",
        "30 PTR#F% = 1",
        "40 BPUT#F%, 90",
        "50 CLOSE#F%",
        &open_in,
        "70 PRINT EXT#G%",
        "80 PRINT BGET#G%; BGET#G%; BGET#G%",
        "90 CLOSE#G%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         0\n         3\n        659067\n");
    assert_eq!(std::fs::read(&file).expect("file is read"), b"AZC");
}

#[test]
fn test_openup_reads_bytes_and_gives_size() {
    let file = temp_file("openup_read");
    std::fs::write(&file, b"ABC").expect("file is written");
    let open_up = format!("10 F% = OPENUP(\"{}\")", file);
    let (result, output) = run(&[
        &open_up,
        "20 PRINT EXT#F%",
        "30 PRINT BGET#F%",
        "40 BPUT#F%, 89",
        "50 PRINT PTR#F%; \" \"; EOF#F%",
        "60 CLOSE#F%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         3\n        65\n         2 0\n");
    assert_eq!(std::fs::read(&file).expect("file is read"), b"AYC");
}

#[test]
fn test_openup_of_missing_file_gives_zero() {
    let file = temp_file("openup_missing");
    let open_up = format!("10 F% = OPENUP(\"{}\")", file);
    let (result, output) = run(&[&open_up, "20 PRINT F%"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         0\n");
    assert!(!std::path::Path::new(&file).exists());
}

#[test]
fn test_close_zero_after_missing_openin_gives_no_error() {
    let file = temp_file("close_zero_missing");
    let open_in = format!("10 F% = OPENIN(\"{}\")", file);
    let (result, output) = run(&[&open_in, "20 CLOSE#F%", "30 PRINT \"OK\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "OK\n");
}

#[test]
fn test_close_zero_closes_all_open_files() {
    let file = temp_file("close_zero_all");
    std::fs::write(&file, b"ABC").expect("file is written");
    let open_in = format!("10 F% = OPENIN(\"{}\")", file);
    let open_up = format!("20 G% = OPENUP(\"{}\")", file);
    let (result, _) = run(&[&open_in, &open_up, "30 CLOSE#0", "40 PRINT BGET#G%"]);

    assert_eq!(result, Err("Channel at line 40".to_string()));
}

#[test]
fn test_other_statement_with_channel_zero_gives_channel_error() {
    let (result, _) = run(&["10 BPUT#0, 65"]);

    assert_eq!(result, Err("Channel at line 10".to_string()));
}

#[test]
fn test_lower_case_names_still_work_as_variable_and_array() {
    let (result, output) = run(&[
        "10 openup = 5 : openin = 6",
        "20 DIM openup(3), openin(3)",
        "30 openup(1) = 7 : openin(1) = 8",
        "40 PRINT openup; openin; openup(1); openin(1)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         5678\n");
}
