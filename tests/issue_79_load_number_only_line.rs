//! Issue 79: LOAD does not store a file line that has only a line number.

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
    assert!(output.contains("1\n3\n"), "{}", output);
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
