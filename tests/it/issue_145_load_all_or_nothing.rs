//! Issue 145: a LOAD or CHAIN of a file with a bad line keeps the program in memory.

use crate::common::run_in_dir as run;
use std::path::PathBuf;

const UNCHANGED: &str = "(program in memory unchanged)";

/// Make an empty directory for one test, with one program file in it.
fn temp_dir_with_file(name: &str, content: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bbc_issue_145_{}_{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir is created");
    std::fs::write(dir.join("prog.bas"), content).expect("program file is written");
    dir
}

#[test]
fn test_failed_load_keeps_old_program() {
    let dir = temp_dir_with_file("load_bad", "10 PRINT 1\n70000 PRINT 2\n");

    let (status, output) = run(&dir, &["5 PRINT 9", "LOAD \"prog.bas\"", "LIST", "EXIT"]);

    assert!(status.success());
    assert!(
        output.contains(&format!(
            "Error: Parse error at line 2: Syntax error: Line number out of range {}\n",
            UNCHANGED
        )),
        "{}",
        output
    );
    assert!(output.contains("> 5 PRINT 9\n"), "{}", output);
    assert!(!output.contains("10 PRINT 1"), "{}", output);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_failed_load_keeps_variables_and_handler() {
    let dir = temp_dir_with_file("load_vars", "10 PRINT 1\n70000 PRINT 2\n");

    let (status, output) = run(
        &dir,
        &[
            "10 ON ERROR PRINT \"HANDLED\" : END",
            "20 X = 7",
            "30 END",
            "40 Y = 1 / 0",
            "RUN",
            "LOAD \"prog.bas\"",
            "PRINT X",
            "GOTO 40",
            "EXIT",
        ],
    );

    assert!(status.success());
    assert!(output.contains(UNCHANGED), "{}", output);
    assert!(output.contains("> 7\n"), "{}", output);
    assert!(output.contains("HANDLED"), "{}", output);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_line_with_no_number_keeps_old_program() {
    let dir = temp_dir_with_file("load_no_number", "10 PRINT 1\nPRINT 2\n");

    let (status, output) = run(&dir, &["5 PRINT 9", "LOAD \"prog.bas\"", "LIST", "EXIT"]);

    assert!(status.success());
    assert!(
        output.contains(&format!(
            "Error: Line 2 has no line number: PRINT 2 {}\n",
            UNCHANGED
        )),
        "{}",
        output
    );
    assert!(output.contains("> 5 PRINT 9\n"), "{}", output);
    assert!(!output.contains("10 PRINT 1"), "{}", output);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_good_load_replaces_program() {
    let dir = temp_dir_with_file("load_good", "10 PRINT 1\n20 PRINT 2\n");

    let (status, output) = run(&dir, &["5 PRINT 9", "LOAD \"prog.bas\"", "LIST", "EXIT"]);

    assert!(status.success());
    assert!(output.contains("Loaded from prog.bas"), "{}", output);
    assert!(output.contains("10 PRINT 1\n20 PRINT 2\n"), "{}", output);
    assert!(!output.contains("5 PRINT 9\n>"), "{}", output);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_failed_chain_runs_nothing() {
    let dir = temp_dir_with_file("chain_bad", "10 PRINT 111\n70000 PRINT 222\n");

    let (status, output) = run(&dir, &["5 PRINT 999", "CHAIN \"prog.bas\"", "EXIT"]);

    assert!(status.success());
    assert!(output.contains(UNCHANGED), "{}", output);
    assert!(!output.contains("111"), "{}", output);
    assert!(!output.contains("999"), "{}", output);
    std::fs::remove_dir_all(&dir).ok();
}
