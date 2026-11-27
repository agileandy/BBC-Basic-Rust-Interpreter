//! Issue 5: LOAD and SAVE use the file name as given when it has an extension.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const TIME_LIMIT: Duration = Duration::from_secs(5);

/// Start the interpreter in `dir`, send the commands, and return the exit status and the output.
fn run(dir: &Path, commands: &[&str]) -> (std::process::ExitStatus, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_bbc-basic-interpreter"))
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("interpreter starts");

    {
        let mut stdin = child.stdin.take().expect("stdin is piped");
        for command in commands {
            writeln!(stdin, "{}", command).expect("command is written");
        }
    }

    let deadline = Instant::now() + TIME_LIMIT;
    let status = loop {
        if let Some(status) = child.try_wait().expect("child status is read") {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().ok();
            child.wait().ok();
            panic!("interpreter did not end within {:?}", TIME_LIMIT);
        }
        std::thread::sleep(Duration::from_millis(20));
    };

    let output = child.wait_with_output().expect("output is read");
    (status, String::from_utf8_lossy(&output.stdout).into_owned())
}

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
