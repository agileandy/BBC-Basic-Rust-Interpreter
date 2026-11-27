//! Tests for the JSON extension functions LOADJSON$, SAVEJSON and JSONVALID,
//! and for the maximum length of a string (issue #102).

use std::fs;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;

/// The ERR number of "Bad JSON".
const BAD_JSON_ERR: &str = "150";

/// Store the source lines, run the program and return the result and the
/// output. The run is in a thread with a time limit, so that a loop defect
/// fails the test and does not hang the test run.
fn run(source: &[&str]) -> (Result<(), String>, String) {
    let source: Vec<String> = source.iter().map(|line| line.to_string()).collect();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut executor = Executor::new();
        let mut program = ProgramStore::new();
        for line in &source {
            program.store_line(tokenize(line).expect("line tokenizes"));
        }
        let result = run_program(&mut executor, &mut program);
        let _ = sender.send((result, executor.get_output().to_string()));
    });
    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("the program ran for more than 5 seconds")
}

/// Make an empty directory for one test, and return its path.
fn temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "bbc_basic_issue_102_{}_{}",
        std::process::id(),
        test_name
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temp dir is made");
    dir
}

/// Write a file in the test directory and return its path as text.
fn write_file(dir: &std::path::Path, name: &str, content: &str) -> String {
    let path = dir.join(name);
    fs::write(&path, content).expect("file is written");
    path.to_string_lossy().into_owned()
}

#[test]
fn test_loadjson_gives_compact_text_of_nested_values() {
    let dir = temp_dir("load_nested");
    let file = write_file(
        &dir,
        "data.json",
        "{\n  \"name\": \"Café \\\"Q\\\"\",\n  \"list\": [1, 2, [3, 4]],\n  \"nested\": { \"ok\": true, \"none\": null }\n}\n",
    );
    let (result, output) = run(&[&format!("10 A$ = LOADJSON$(\"{file}\")"), "20 PRINT A$"]);
    assert_eq!(result, Ok(()));
    assert_eq!(
        output,
        "{\"name\":\"Café \\\"Q\\\"\",\"list\":[1,2,[3,4]],\"nested\":{\"ok\":true,\"none\":null}}\n"
    );
}

#[test]
fn test_savejson_then_loadjson_gives_the_same_text() {
    let dir = temp_dir("save_load");
    let file = dir.join("out.json").to_string_lossy().into_owned();
    let (result, output) = run(&[
        &format!("10 F$ = \"{file}\""),
        "20 J$ = \"{\"\"a\"\":[1,2],\"\"b\"\":{\"\"c\"\":\"\"x\"\"}}\"",
        "30 SAVEJSON F$, J$",
        "40 B$ = LOADJSON$(F$)",
        "50 PRINT B$",
        "60 PRINT B$ = J$",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "{\"a\":[1,2],\"b\":{\"c\":\"x\"}}\n-1\n");
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        "{\n  \"a\": [\n    1,\n    2\n  ],\n  \"b\": {\n    \"c\": \"x\"\n  }\n}\n"
    );
}

#[test]
fn test_loadjson_and_savejson_keep_the_key_order_of_the_file() {
    let dir = temp_dir("key_order");
    let file = write_file(
        &dir,
        "order.json",
        "{\"z\": 1, \"a\": {\"y\": 2, \"b\": 3}, \"m\": 4}",
    );
    let (result, output) = run(&[
        &format!("10 F$ = \"{file}\""),
        "20 A$ = LOADJSON$(F$)",
        "30 PRINT A$",
        "40 SAVEJSON F$, A$",
        "50 PRINT LOADJSON$(F$)",
    ]);
    assert_eq!(result, Ok(()));
    let expected = "{\"z\":1,\"a\":{\"y\":2,\"b\":3},\"m\":4}\n";
    assert_eq!(output, format!("{expected}{expected}"));
}

#[test]
fn test_savejson_file_name_can_have_a_comma_in_brackets() {
    let dir = temp_dir("save_comma");
    let file = dir.join("c.json").to_string_lossy().into_owned();
    let (result, _) = run(&[
        &format!("10 F$ = \"{file}XYZ\""),
        "20 SAVEJSON LEFT$(F$, LEN(F$) - 3), \"[true]\"",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(fs::read_to_string(&file).unwrap(), "[\n  true\n]\n");
}

#[test]
fn test_loadjson_of_missing_file_gives_file_not_found() {
    let (result, _) = run(&["10 A$ = LOADJSON$(\"/no/such/dir/x.json\")"]);
    assert_eq!(
        result,
        Err("File not found: /no/such/dir/x.json at line 10".to_string())
    );
}

#[test]
fn test_on_error_catches_loadjson_of_missing_file() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 A$ = LOADJSON$(\"/no/such/dir/x.json\")",
        "30 END",
        "100 PRINT \"CAUGHT \"; ERR",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "CAUGHT 255\n");
}

#[test]
fn test_loadjson_of_bad_file_gives_bad_json() {
    let dir = temp_dir("load_bad");
    let file = write_file(&dir, "bad.json", "{bad");
    let (result, _) = run(&[&format!("10 A$ = LOADJSON$(\"{file}\")")]);
    assert_eq!(
        result,
        Err("Bad JSON: key must be a string at line 1 column 2 at line 10".to_string())
    );
}

#[test]
fn test_on_error_catches_loadjson_of_bad_file() {
    let dir = temp_dir("load_bad_caught");
    let file = write_file(&dir, "bad.json", "[1,]");
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        &format!("20 A$ = LOADJSON$(\"{file}\")"),
        "30 END",
        "100 PRINT ERR",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), BAD_JSON_ERR);
}

#[test]
fn test_savejson_of_bad_text_gives_bad_json_and_keeps_the_file() {
    let dir = temp_dir("save_bad");
    let file = write_file(&dir, "keep.json", "[1]\n");
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        &format!("20 SAVEJSON \"{file}\", \"{{bad\""),
        "30 END",
        "100 PRINT ERR",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), BAD_JSON_ERR);
    assert_eq!(fs::read_to_string(&file).unwrap(), "[1]\n");
}

#[test]
fn test_jsonvalid_for_5_texts() {
    let (result, output) = run(&[
        "10 PRINT JSONVALID(\"{\"\"a\"\":1}\")",
        "20 PRINT JSONVALID(\"[1, 2, 3]\")",
        "30 PRINT JSONVALID(\"{bad\")",
        "40 PRINT JSONVALID(\"\")",
        "50 PRINT JSONVALID(\"[1,]\")",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "-1\n-1\n0\n0\n0\n");
}

#[test]
fn test_loadjson_longer_than_the_string_limit_gives_string_too_long() {
    let dir = temp_dir("load_long");
    let file = write_file(&dir, "long.json", &format!("\"{}\"", "A".repeat(70000)));
    let (result, _) = run(&[&format!("10 PRINT LEN(LOADJSON$(\"{file}\"))")]);
    assert_eq!(result, Err("String too long at line 10".to_string()));
}

#[test]
fn test_string_of_1000_characters_in_a_variable_works() {
    let (result, output) = run(&["10 A$ = STRING$(1000, \"A\")", "20 PRINT LEN(A$)"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "1000\n");
}

#[test]
fn test_string_of_70000_characters_in_a_variable_gives_string_too_long() {
    let (result, _) = run(&["10 A$ = STRING$(35000, \"A\") + STRING$(35000, \"B\")"]);
    assert_eq!(result, Err("String too long at line 10".to_string()));
}

#[test]
fn test_variables_with_the_new_names_still_work() {
    let (result, output) = run(&[
        "10 SAVEJSON = 5",
        "20 LOADJSON$ = \"a\"",
        "30 JSONVALID = 3",
        "40 PRINT SAVEJSON; LOADJSON$; JSONVALID",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "5a3\n");
}

#[test]
fn test_arrays_with_the_new_names_still_work() {
    let (result, output) = run(&[
        "10 DIM LOADJSON$(2), JSONVALID(2), SAVEJSON(2)",
        "20 LOADJSON$(1) = \"q\"",
        "30 JSONVALID(1) = 7",
        "40 SAVEJSON(1) = 8",
        "50 PRINT LOADJSON$(1); JSONVALID(1); SAVEJSON(1)",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "q78\n");
}
