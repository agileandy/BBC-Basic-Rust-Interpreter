//! Tests for the ERR number of "Too many open files" (issue #130).
//!
//! A test cannot open 255 real files, so each test lowers the open file
//! limit of the executor.

use std::path::PathBuf;

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;

/// Make an empty directory that only this test uses.
fn temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "bbc_issue_130_{}_{}",
        test_name,
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp directory is made");
    dir
}

/// Run the program with an open file limit of `limit`, on a thread with a
/// time limit of 5 seconds.
fn run_with_file_limit(limit: usize, source: Vec<String>) -> (Result<(), String>, String) {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut executor = Executor::new();
        executor.set_open_file_limit(limit);
        let mut program = ProgramStore::new();
        for line in &source {
            program.store_line(tokenize(line).expect("line tokenizes"));
        }
        let result = run_program(&mut executor, &mut program);
        let _ = sender.send((result, executor.get_output().to_string()));
    });
    receiver
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("program stops in 5 seconds")
}

#[test]
fn test_the_open_after_the_limit_gives_err_192_and_a_close_frees_a_channel() {
    let dir = temp_dir("limit_two");
    let file = |name: &str| dir.join(name).display().to_string();
    let source = vec![
        "10 ON ERROR GOTO 100".to_string(),
        format!("20 A% = OPENOUT(\"{}\")", file("one")),
        format!("30 B% = OPENOUT(\"{}\")", file("two")),
        format!("40 C% = OPENOUT(\"{}\")", file("three")),
        "50 PRINT \"no error\"".to_string(),
        "60 END".to_string(),
        "100 PRINT ERR; \" \"; ERL".to_string(),
        "110 CLOSE# A%".to_string(),
        format!("120 C% = OPENOUT(\"{}\")", file("three")),
        "130 PRINT C% > 0".to_string(),
        "140 CLOSE# B%".to_string(),
        "150 CLOSE# C%".to_string(),
    ];
    let (result, output) = run_with_file_limit(2, source);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "192 40\n-1\n");
}

#[test]
fn test_openin_after_the_limit_gives_err_192() {
    let dir = temp_dir("openin");
    let file = dir.join("one").display().to_string();
    let source = vec![
        "10 ON ERROR PRINT ERR : END".to_string(),
        format!("20 A% = OPENOUT(\"{}\")", file),
        format!("30 B% = OPENIN(\"{}\")", file),
        "40 PRINT \"no error\"".to_string(),
    ];
    let (result, output) = run_with_file_limit(1, source);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "192\n");
}

#[test]
fn test_with_no_handler_the_error_text_names_the_line() {
    let dir = temp_dir("no_handler");
    let source = vec![
        format!("10 A% = OPENOUT(\"{}\")", dir.join("one").display()),
        format!("20 B% = OPENOUT(\"{}\")", dir.join("two").display()),
    ];
    let (result, _) = run_with_file_limit(1, source);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(result, Err("Too many open files at line 20".to_string()));
}
