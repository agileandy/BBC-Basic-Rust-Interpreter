//! Issue 16: *CAT lists .bas files, the SAVE error names the file, and the examples load.

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
    let dir = std::env::temp_dir().join(format!("bbc_issue_16_{}_{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir is created");
    dir
}

#[test]
fn test_cat_lists_bas_and_bbas_files_only() {
    let dir = temp_dir("cat");
    std::fs::write(dir.join("first.bas"), "10 PRINT 1\n").unwrap();
    std::fs::write(dir.join("second.bbas"), "10 PRINT 2\n").unwrap();
    std::fs::write(dir.join("notes.txt"), "text\n").unwrap();

    let (status, output) = run(&dir, &["*CAT", "EXIT"]);

    assert!(status.success());
    assert!(output.contains("first.bas "), "{}", output);
    assert!(output.contains("second.bbas"), "{}", output);
    assert!(!output.contains("notes.txt"), "{}", output);
    assert!(output.contains("2 file(s)"), "{}", output);
    let first = output.find("first.bas").unwrap();
    let second = output.find("second.bbas").unwrap();
    assert!(first < second, "{}", output);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_save_to_missing_directory_gives_error_with_path() {
    let dir = temp_dir("save_missing");

    let (status, output) = run(&dir, &["10 PRINT \"X\"", "SAVE \"nodir/prog.bas\"", "EXIT"]);

    assert!(status.success());
    assert!(
        output.contains("Error: Failed to create file nodir/prog.bas:"),
        "{}",
        output
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// LOAD and RUN one example file from the repository root.
fn load_and_run_example(name: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = format!("examples/{}", name);
    let load = format!("LOAD \"{}\"", path);

    let (status, output) = run(root, &[&load, "RUN", "EXIT"]);

    assert!(status.success());
    assert!(
        output.contains(&format!("Loaded from {}", path)),
        "{}",
        output
    );
    assert!(!output.contains("Error: "), "{}", output);
    output
}

#[test]
fn test_load_and_run_while_example() {
    let output = load_and_run_example("test_while.bas");

    assert!(
        output.contains("All WHILE loop tests completed!"),
        "{}",
        output
    );
}

#[test]
fn test_load_and_run_error_handling_example() {
    let output = load_and_run_example("test_error_handling.bas");

    assert!(output.contains("Error caught!"), "{}", output);
    assert!(output.contains("Y =42"), "{}", output);
}
