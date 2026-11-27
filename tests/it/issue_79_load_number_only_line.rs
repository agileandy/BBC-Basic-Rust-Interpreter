//! Issue 79: LOAD does not store a file line that has only a line number.

use crate::common::run_in_dir as run;
use std::path::PathBuf;

/// Make an empty directory for one test, with one program file in it.
fn temp_dir_with_file(name: &str, content: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bbc_issue_79_{}_{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir is created");
    std::fs::write(dir.join("prog.bas"), content).expect("program file is written");
    dir
}

#[test]
fn test_load_skips_line_with_only_a_number() {
    let dir = temp_dir_with_file("skip", "10 PRINT 1\n20\n30 PRINT 3\n");

    let (status, output) = run(&dir, &["LOAD \"prog.bas\"", "LIST", "RUN", "EXIT"]);

    assert!(status.success());
    assert!(output.contains("Loaded from prog.bas"), "{}", output);
    assert!(output.contains("10 PRINT 1\n30 PRINT 3\n"), "{}", output);
    assert!(!output.contains("\n20\n"), "{}", output);
    assert!(output.contains("         1\n         3\n"), "{}", output);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_load_number_only_line_deletes_earlier_line() {
    let dir = temp_dir_with_file("delete", "10 PRINT 1\n10\n");

    let (status, output) = run(&dir, &["LOAD \"prog.bas\"", "LIST", "EXIT"]);

    assert!(status.success());
    assert!(output.contains("Loaded from prog.bas"), "{}", output);
    assert!(!output.contains("10 PRINT 1"), "{}", output);
    assert!(output.contains("No program"), "{}", output);
    std::fs::remove_dir_all(&dir).ok();
}
