//! Issue 106: LISTDIR$, MKDIR and RMDIR. These are modern extensions, not
//! BBC BASIC. Old programs that use these names for variables still work.

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;
use std::path::PathBuf;

/// Store the source lines, run the program and return the result and the output.
fn run(source: &[&str]) -> (Result<(), String>, String) {
    let mut executor = Executor::new();
    let mut program = ProgramStore::new();
    for line in source {
        program.store_line(tokenize(line).expect("line tokenizes"));
    }
    let result = run_program(&mut executor, &mut program);
    (result, executor.get_output().to_string())
}

/// Make a new empty directory for one test and return its path as text.
fn fresh_dir(name: &str) -> (PathBuf, String) {
    let dir = std::env::temp_dir().join(format!("bbc_issue_106_{}_{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir(&dir).expect("test directory is made");
    let text = dir.to_str().expect("path is text").to_string();
    (dir, text)
}

#[test]
fn test_mkdir_openout_and_listdir_show_the_file_and_the_sub_directory() {
    let (dir, path) = fresh_dir("list");
    let (result, output) = run(&[
        &format!("10 MKDIR \"{}/data\"", path),
        &format!("20 F% = OPENOUT(\"{}/notes.txt\")", path),
        "30 CLOSE#F%",
        &format!("40 PRINT LISTDIR$(\"{}\")", path),
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "data/\nnotes.txt\n");
    assert!(dir.join("data").is_dir());
}

#[test]
fn test_rmdir_of_a_directory_with_content_gives_bad_directory_and_removes_nothing() {
    let (dir, path) = fresh_dir("rmdir_full");
    std::fs::create_dir(dir.join("full")).unwrap();
    std::fs::write(dir.join("full").join("data"), "x").unwrap();

    let (result, output) = run(&[&format!("10 RMDIR \"{}/full\"", path)]);

    assert_eq!(result, Err("Bad directory at line 10".to_string()));
    assert_eq!(output, "");
    assert!(dir.join("full").join("data").is_file());
}

#[test]
fn test_rmdir_of_an_empty_directory_removes_it() {
    let (dir, path) = fresh_dir("rmdir_empty");
    let (result, output) = run(&[
        &format!("10 MKDIR \"{}/empty\"", path),
        &format!("20 RMDIR \"{}/empty\"", path),
        "30 PRINT \"DONE\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "DONE\n");
    assert!(!dir.join("empty").exists());
}

#[test]
fn test_rmdir_of_a_file_gives_bad_directory_and_keeps_the_file() {
    let (dir, path) = fresh_dir("rmdir_file");
    std::fs::write(dir.join("f"), "x").unwrap();

    let (result, _) = run(&[&format!("10 RMDIR \"{}/f\"", path)]);

    assert_eq!(result, Err("Bad directory at line 10".to_string()));
    assert!(dir.join("f").is_file());
}

#[test]
fn test_a_path_that_is_not_there_gives_file_not_found() {
    let (_, path) = fresh_dir("missing");
    let missing = format!("{}/missing", path);

    let (result, _) = run(&[&format!("10 PRINT LISTDIR$(\"{}\")", missing)]);

    assert_eq!(
        result,
        Err(format!("File not found: {} at line 10", missing))
    );
}

#[test]
fn test_old_program_with_variables_named_like_the_new_words_still_works() {
    let (result, output) = run(&[
        "10 MKDIR% = 3",
        "20 MKDIR = 4",
        "30 RMDIR = 5",
        "40 LISTDIR$ = \"L\"",
        "50 PRINT MKDIR%; MKDIR; RMDIR; LISTDIR$",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "345L\n");
}

#[test]
fn test_old_program_with_arrays_named_like_the_new_words_still_works() {
    let (result, output) = run(&[
        "10 DIM LISTDIR$(2), MKDIR(2), RMDIR%(2)",
        "20 LISTDIR$(1) = \"A\"",
        "30 MKDIR(1) = 6",
        "40 RMDIR%(2) = 7",
        "50 PRINT LISTDIR$(1); MKDIR(1); RMDIR%(2)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "A67\n");
}

/// Run one directory statement with ON ERROR and return what the handler prints.
fn caught(statement: &str) -> String {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        &format!("20 {}", statement),
        "30 PRINT \"NOT CAUGHT\"",
        "40 END",
        "100 PRINT ERR; \" \"; ERL",
    ]);
    assert_eq!(result, Ok(()));
    output
}

#[test]
fn test_on_error_catches_each_directory_error() {
    let (dir, path) = fresh_dir("on_error");
    std::fs::create_dir(dir.join("full")).unwrap();
    std::fs::write(dir.join("full").join("data"), "x").unwrap();
    std::fs::write(dir.join("f"), "x").unwrap();

    // ERR 151 is Bad directory. BBC BASIC does not use 151.
    assert_eq!(caught(&format!("RMDIR \"{}/full\"", path)), "151 20\n");
    assert_eq!(caught(&format!("RMDIR \"{}/f\"", path)), "151 20\n");
    assert_eq!(caught(&format!("MKDIR \"{}/f\"", path)), "151 20\n");
    assert_eq!(
        caught(&format!("A$ = LISTDIR$(\"{}/f\")", path)),
        "151 20\n"
    );
    assert_eq!(caught(&format!("RMDIR \"{}/missing\"", path)), "255 20\n");
    assert_eq!(
        caught(&format!("MKDIR \"{}/missing/new\"", path)),
        "255 20\n"
    );
    assert_eq!(
        caught(&format!("A$ = LISTDIR$(\"{}/missing\")", path)),
        "255 20\n"
    );
}
