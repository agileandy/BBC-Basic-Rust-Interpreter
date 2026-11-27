//! Issue 28: *CAT <directory> lists the program files of a named directory.

use crate::common::run_in_dir as run;
use std::path::{Path, PathBuf};

/// Make a directory for one test, with a sub-directory `sub` that holds two program files.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bbc_issue_28_{}_{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sub")).expect("temp dir is created");
    std::fs::write(dir.join("top.bas"), "10 PRINT 0\n").unwrap();
    std::fs::write(dir.join("sub").join("alpha.bas"), "10 PRINT 1\n").unwrap();
    std::fs::write(dir.join("sub").join("beta.bbas"), "10 PRINT 2\n").unwrap();
    std::fs::write(dir.join("sub").join("notes.txt"), "text\n").unwrap();
    dir
}

fn assert_lists_sub(output: &str) {
    assert!(output.contains("Catalog: sub"), "{}", output);
    assert!(output.contains("alpha.bas "), "{}", output);
    assert!(output.contains("beta.bbas"), "{}", output);
    assert!(!output.contains("notes.txt"), "{}", output);
    assert!(!output.contains("top.bas"), "{}", output);
    assert!(output.contains("2 file(s)"), "{}", output);
}

#[test]
fn test_cat_named_directory_lists_its_files() {
    let dir = temp_dir("named");

    let (status, output) = run(&dir, &["*CAT sub", "EXIT"]);

    assert!(status.success());
    assert_lists_sub(&output);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_cat_quoted_directory_lists_its_files() {
    let dir = temp_dir("quoted");

    let (status, output) = run(&dir, &["*cat \"sub\"", "EXIT"]);

    assert!(status.success());
    assert_lists_sub(&output);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_cat_missing_directory_gives_error_and_continues() {
    let dir = temp_dir("missing");

    let (status, output) = run(&dir, &["*CAT missing", "PRINT \"NEXT\"", "EXIT"]);

    assert!(status.success());
    assert!(
        output.contains("Error: Directory not found: missing"),
        "{}",
        output
    );
    assert!(output.contains("NEXT"), "{}", output);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_cat_file_path_gives_error() {
    let dir = temp_dir("file");

    let (status, output) = run(&dir, &["*CAT top.bas", "EXIT"]);

    assert!(status.success());
    assert!(
        output.contains("Error: Directory not found: top.bas"),
        "{}",
        output
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_cat_with_no_argument_is_unchanged() {
    let dir = temp_dir("plain");

    let (status, output) = run(&dir, &["*CAT", "EXIT"]);

    assert!(status.success());
    assert!(output.contains("Catalog:\n"), "{}", output);
    assert!(output.contains("top.bas"), "{}", output);
    assert!(!output.contains("alpha.bas"), "{}", output);
    assert!(output.contains("1 file(s)"), "{}", output);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_cat_examples_from_project_directory() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    let (status, output) = run(root, &["*CAT examples", "EXIT"]);

    assert!(status.success());
    assert!(output.contains("Catalog: examples"), "{}", output);
    assert!(output.contains("sinewave.bas"), "{}", output);
    assert!(!output.contains("Error: "), "{}", output);
}

#[test]
fn test_help_shows_cat_directory_form() {
    let dir = temp_dir("help");

    let (status, output) = run(&dir, &["HELP", "EXIT"]);

    assert!(status.success());
    assert!(output.contains("*CAT directory"), "{}", output);
    std::fs::remove_dir_all(&dir).ok();
}
