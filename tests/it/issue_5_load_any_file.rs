//! Issue 5: LOAD and SAVE use the file name as given when it has an extension.

use crate::common::run_in_dir as run;
use std::path::{Path, PathBuf};

/// Make an empty directory for one test.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bbc_issue_5_{}_{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir is created");
    dir
}

#[test]
fn test_load_example_with_bas_extension_and_run() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    let (status, output) = run(root, &["LOAD \"examples/test_if.bas\"", "RUN", "EXIT"]);

    assert!(status.success());
    assert!(
        output.contains("Loaded from examples/test_if.bas"),
        "{}",
        output
    );
    assert!(output.contains("X is greater than 5"), "{}", output);
    assert!(output.contains("Y is small"), "{}", output);
}

#[test]
fn test_save_and_load_name_without_extension_uses_bbas() {
    let dir = temp_dir("no_ext");

    let (status, output) = run(
        &dir,
        &[
            "10 PRINT \"SAVED PROGRAM\"",
            "SAVE \"prog\"",
            "NEW",
            "LOAD \"prog\"",
            "RUN",
            "EXIT",
        ],
    );

    assert!(status.success());
    assert!(dir.join("prog.bbas").is_file(), "{}", output);
    assert!(output.contains("Saved to prog.bbas"), "{}", output);
    assert!(output.contains("Loaded from prog.bbas"), "{}", output);
    assert!(output.contains("SAVED PROGRAM"), "{}", output);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_save_name_with_extension_uses_name_as_given() {
    let dir = temp_dir("with_ext");

    let (status, output) = run(
        &dir,
        &[
            "10 PRINT \"BAS PROGRAM\"",
            "SAVE \"prog.bas\"",
            "NEW",
            "LOAD \"prog.bas\"",
            "RUN",
            "EXIT",
        ],
    );

    assert!(status.success());
    assert!(dir.join("prog.bas").is_file(), "{}", output);
    assert!(!dir.join("prog.bas.bbas").exists(), "{}", output);
    assert!(output.contains("BAS PROGRAM"), "{}", output);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_load_missing_file_gives_error_with_path() {
    let dir = temp_dir("missing");

    let (status, output) = run(&dir, &["LOAD \"missing.bas\"", "EXIT"]);

    assert!(status.success());
    assert!(
        output.contains("Error: Failed to read file missing.bas:"),
        "{}",
        output
    );
    assert!(output.contains("Goodbye!"), "{}", output);
    std::fs::remove_dir_all(&dir).ok();
}
