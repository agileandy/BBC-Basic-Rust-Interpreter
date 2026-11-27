//! Issue #185: `INSTALL file$` loads a library file. The program then calls
//! the procedures and functions of the library as its own. Each test writes
//! its library files to its own temporary directory.

use crate::common::{run_binary_read_first, run_in_dir, run_with_limit};
use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::{run_program, run_typed_line};
use bbc_basic_interpreter::tokenizer::tokenize;
use std::fs;
use std::path::{Path, PathBuf};

/// A new empty directory with a name that only this test uses.
fn test_dir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bbc_issue185_{}_{}", test, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("test directory is made");
    dir
}

/// Write the lines to `dir/file` and give the path as text.
fn write_library(dir: &Path, file: &str, lines: &[&str]) -> String {
    let path = dir.join(file);
    fs::write(&path, lines.join("\n") + "\n").expect("library file is written");
    path.to_string_lossy().into_owned()
}

/// Run the program lines with the time limit of `run_with_limit`.
fn run(source: &[String]) -> (Result<(), String>, String) {
    let lines: Vec<&str> = source.iter().map(String::as_str).collect();
    run_with_limit(&lines)
}

/// The program line `<number> INSTALL "<path>"`.
fn install(number: u16, path: &str) -> String {
    format!("{} INSTALL \"{}\"", number, path)
}

fn lines(source: &[&str]) -> Vec<String> {
    source.iter().map(|line| line.to_string()).collect()
}

#[test]
fn a_program_calls_a_procedure_and_functions_of_a_library() {
    let dir = test_dir("calls");
    let path = write_library(
        &dir,
        "greetlib.bbas",
        &[
            "10 DEF PROCgreet(N$)",
            "20 LOCAL T$",
            "30 T$=\"Hello \"+N$",
            "40 PRINT T$",
            "50 ENDPROC",
            "60 DEF FNsq(X)=X*X",
            "70 DEF FNcube(X)",
            "80 LOCAL R",
            "90 R=X*X*X",
            "100 =R",
        ],
    );
    let mut source = vec![install(10, &path)];
    source.extend(lines(&[
        "15 T$=\"keep\"",
        "20 PROCgreet(\"Ann\")",
        "30 PRINT ;FNsq(4)",
        "40 PRINT ;FNcube(2)",
        "45 PRINT T$",
        "50 END",
    ]));

    let (result, output) = run(&source);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "Hello Ann\n16\n8\nkeep\n");
}

#[test]
fn the_line_numbers_of_a_library_can_be_the_same_as_those_of_the_program() {
    let dir = test_dir("same_lines");
    let path = write_library(
        &dir,
        "samelib.bbas",
        &["10 DEF PROCa", "20 PRINT \"lib 20\"", "30 ENDPROC"],
    );
    let mut source = vec![install(10, &path)];
    source.extend(lines(&["20 PROCa", "30 PRINT \"prog 30\"", "40 END"]));

    let (result, output) = run(&source);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "lib 20\nprog 30\n");
}

#[test]
fn the_extension_is_added_when_the_name_has_none() {
    let dir = test_dir("extension");
    write_library(
        &dir,
        "extlib.bbas",
        &["10 DEF PROCe", "20 PRINT \"ext\"", "30 ENDPROC"],
    );
    let path = dir.join("extlib").to_string_lossy().into_owned();
    let mut source = vec![install(10, &path)];
    source.extend(lines(&["20 PROCe"]));

    let (result, output) = run(&source);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "ext\n");
}

#[test]
fn code_of_a_library_outside_a_def_does_not_run() {
    let dir = test_dir("outside");
    let path = write_library(
        &dir,
        "outlib.bbas",
        &[
            "10 PRINT \"outside\"",
            "20 DEF PROCa",
            "30 PRINT \"in a\"",
            "40 ENDPROC",
            "50 PRINT \"after\"",
        ],
    );
    let mut source = vec![install(10, &path)];
    source.extend(lines(&["20 PROCa", "30 PRINT \"end\""]));

    let (result, output) = run(&source);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "in a\nend\n");
}

#[test]
fn a_second_install_of_the_same_file_does_not_load_it_again() {
    let dir = test_dir("again");
    let library = ["10 DEF PROCv", "20 PRINT \"version 1\"", "30 ENDPROC"];
    let path = write_library(&dir, "againlib.bbas", &library);
    let mut executor = Executor::new();
    let mut program = ProgramStore::new();
    for line in [
        install(10, &path),
        install(20, &path),
        "30 PROCv".to_string(),
    ] {
        program.store_line(tokenize(&line).expect("line tokenizes"));
    }
    assert_eq!(run_program(&mut executor, &mut program), Ok(()));

    // A changed file: an INSTALL of the same name keeps the first version
    write_library(
        &dir,
        "againlib.bbas",
        &["10 DEF PROCv", "20 PRINT \"version 2\"", "30 ENDPROC"],
    );
    let typed = tokenize(&format!("INSTALL \"{}\" : PROCv", path)).expect("line tokenizes");
    assert!(run_typed_line(&mut executor, &program, &typed).is_ok());

    assert_eq!(executor.get_output(), "version 1\nversion 1\n");
}

#[test]
fn a_routine_of_the_program_with_the_same_name_wins() {
    let dir = test_dir("program_wins");
    let path = write_library(
        &dir,
        "winlib.bbas",
        &[
            "10 DEF PROCa",
            "20 PRINT \"library\"",
            "30 ENDPROC",
            "40 DEF FNv=1",
        ],
    );
    let mut source = vec![install(10, &path)];
    source.extend(lines(&[
        "20 PROCa",
        "30 PRINT ;FNv",
        "40 END",
        "50 DEF PROCa",
        "60 PRINT \"program\"",
        "70 ENDPROC",
        "80 DEF FNv=2",
    ]));

    let (result, output) = run(&source);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "program\n2\n");
}

#[test]
fn the_first_library_that_defines_a_name_wins() {
    let dir = test_dir("first_wins");
    let one = write_library(
        &dir,
        "one.bbas",
        &["10 DEF PROCa", "20 PRINT \"one\"", "30 ENDPROC"],
    );
    let two = write_library(
        &dir,
        "two.bbas",
        &[
            "10 DEF PROCa",
            "20 PRINT \"two\"",
            "30 ENDPROC",
            "40 DEF PROCb",
            "50 PRINT \"two b\"",
            "60 ENDPROC",
        ],
    );
    let mut source = vec![install(10, &one), install(20, &two)];
    source.extend(lines(&["30 PROCa", "40 PROCb"]));

    let (result, output) = run(&source);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "one\ntwo b\n");
}

#[test]
fn a_library_routine_calls_a_routine_of_the_program_and_of_its_library() {
    let dir = test_dir("cross_calls");
    let path = write_library(
        &dir,
        "crosslib.bbas",
        &[
            "10 DEF PROCouter",
            "20 PROCinner",
            "30 PROCprog",
            "40 PRINT ;FNtwice(3)",
            "50 ENDPROC",
            "60 DEF PROCinner",
            "70 PRINT \"inner\"",
            "80 ENDPROC",
            "90 DEF FNtwice(X)=FNprogfn(X)*2",
        ],
    );
    let mut source = vec![install(10, &path)];
    source.extend(lines(&[
        "20 PROCouter",
        "30 END",
        "40 DEF PROCprog",
        "50 PRINT \"prog\"",
        "60 ENDPROC",
        "70 DEF FNprogfn(X)=X+1",
    ]));

    let (result, output) = run(&source);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "inner\nprog\n8\n");
}

#[test]
fn a_library_function_works_in_an_expression_in_print_and_as_an_argument() {
    let dir = test_dir("expressions");
    let path = write_library(
        &dir,
        "factlib.bbas",
        &[
            "10 DEF FNfact(N)",
            "20 LOCAL R",
            "30 IF N<=1 THEN R=1 ELSE R=N*FNfact(N-1)",
            "40 =R",
            "50 DEF FNadd(A,B)=A+B",
        ],
    );
    let mut source = vec![install(10, &path)];
    source.extend(lines(&[
        "20 X=FNfact(5)+1",
        "30 PRINT ;X",
        "40 PRINT ;FNfact(4)",
        "50 PRINT ;FNadd(FNfact(3),2)",
    ]));

    let (result, output) = run(&source);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "121\n24\n8\n");
}

#[test]
fn a_library_procedure_with_a_loop_and_an_early_endproc() {
    let dir = test_dir("loop");
    let path = write_library(
        &dir,
        "looplib.bbas",
        &[
            "10 DEF PROCfind(T)",
            "20 LOCAL I",
            "30 FOR I=1 TO 10",
            "40 IF I=T THEN PRINT \"found \";I : ENDPROC",
            "50 NEXT",
            "60 PRINT \"none\"",
            "70 ENDPROC",
        ],
    );
    let mut source = vec![install(10, &path)];
    source.extend(lines(&[
        "20 PROCfind(3)",
        "30 PROCfind(20)",
        "40 FOR J=1 TO 2",
        "50 PROCfind(J)",
        "60 NEXT",
        "70 END",
    ]));

    let (result, output) = run(&source);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "found 3\nnone\nfound 1\nfound 2\n");
}

#[test]
fn a_jump_with_a_line_number_in_a_library_goes_to_a_line_of_the_library() {
    let dir = test_dir("jumps");
    let path = write_library(
        &dir,
        "jumplib.bbas",
        &[
            "10 DEF PROCj",
            "20 GOTO 40",
            "30 PRINT \"lib 30\"",
            "40 PRINT \"lib 40\"",
            "50 GOSUB 90",
            "60 ENDPROC",
            "90 PRINT \"lib sub\"",
            "100 RETURN",
        ],
    );
    let mut source = vec![install(10, &path)];
    source.extend(lines(&[
        "20 PROCj",
        "30 END",
        "40 PRINT \"prog 40\"",
        "90 PRINT \"prog 90\"",
        "100 RETURN",
    ]));

    let (result, output) = run(&source);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "lib 40\nlib sub\n");
}

#[test]
fn an_error_in_a_library_procedure_gives_the_line_and_the_file() {
    let dir = test_dir("proc_error");
    let path = write_library(
        &dir,
        "badlib.bbas",
        &["10 DEF PROCbad", "20 PRINT ;1/0", "30 ENDPROC"],
    );
    let mut source = vec![install(10, &path)];
    source.extend(lines(&["20 PROCbad"]));

    let (result, _) = run(&source);

    assert_eq!(
        result,
        Err(format!("Division by zero at line 20 in \"{}\"", path))
    );
}

#[test]
fn an_error_in_a_library_function_gives_the_line_and_the_file() {
    let dir = test_dir("fn_error");
    let path = write_library(
        &dir,
        "mathlib.bbas",
        &[
            "10 DEF FNdiv(X)",
            "20 LOCAL Y",
            "30 Y=X/0",
            "40 =Y",
            "50 DEF FNzero(X)=X/0",
        ],
    );
    let mut multi = vec![install(10, &path)];
    multi.extend(lines(&["20 PRINT ;FNdiv(1)"]));
    let mut single = vec![install(10, &path)];
    single.extend(lines(&["20 PRINT ;FNzero(1)"]));

    assert_eq!(
        run(&multi).0,
        Err(format!("Division by zero at line 30 in \"{}\"", path))
    );
    assert_eq!(
        run(&single).0,
        Err(format!("Division by zero at line 50 in \"{}\"", path))
    );
}

#[test]
fn the_handler_of_the_program_catches_an_error_in_a_library() {
    let dir = test_dir("handler");
    let path = write_library(
        &dir,
        "traplib.bbas",
        &[
            "10 DEF PROCbad",
            "20 PRINT ;1/0",
            "30 ENDPROC",
            "100 PRINT \"wrong handler\"",
        ],
    );
    let mut source = lines(&["10 ON ERROR GOTO 100"]);
    source.push(install(20, &path));
    source.extend(lines(&[
        "30 PROCbad",
        "40 END",
        "100 PRINT \"ERL=\";ERL;\" ERR=\";ERR",
        "110 END",
    ]));

    let (result, output) = run(&source);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "ERL=20 ERR=18\n");
}

#[test]
fn on_error_goto_in_a_library_goes_to_a_line_of_the_library() {
    let dir = test_dir("library_handler");
    let path = write_library(
        &dir,
        "ownlib.bbas",
        &[
            "10 DEF PROCtrap",
            "20 ON ERROR GOTO 50",
            "30 PRINT ;1/0",
            "40 ENDPROC",
            "50 PRINT \"library handler\"",
            "60 END",
        ],
    );
    let mut source = vec![install(10, &path)];
    source.extend(lines(&["20 PROCtrap", "50 PRINT \"program 50\""]));

    let (result, output) = run(&source);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "library handler\n");
}

#[test]
fn a_file_that_is_not_there_gives_file_not_found() {
    let dir = test_dir("missing");
    let missing = dir.join("nolib").to_string_lossy().into_owned();
    let mut trapped = lines(&["10 ON ERROR PRINT ;ERR;\" \";ERL : END"]);
    trapped.extend([install(20, &missing), "30 PRINT \"no\"".to_string()]);
    let untrapped = vec![install(20, &missing)];

    let (result, output) = run(&trapped);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "214 20\n");

    assert_eq!(
        run(&untrapped).0,
        Err(format!("File not found: {}.bbas at line 20", missing))
    );
}

#[test]
fn a_library_line_with_a_fault_gives_its_error_at_install() {
    let dir = test_dir("bad_line");
    let path = write_library(
        &dir,
        "faultlib.bbas",
        &["10 DEF PROCa", "20 PRINT (", "30 ENDPROC"],
    );
    let mut source = lines(&["10 PRINT \"before\""]);
    source.extend([install(20, &path), "30 PRINT \"after\"".to_string()]);

    let (result, output) = run(&source);

    assert_eq!(output, "before\n");
    assert_eq!(
        result,
        Err(format!(
            "Syntax error at line 20 in \"{}\": Unexpected end of expression",
            path
        ))
    );
}

#[test]
fn a_library_line_with_no_line_number_gives_bad_library() {
    let dir = test_dir("no_number");
    let path = write_library(&dir, "nonum.bbas", &["10 DEF PROCa", "PRINT \"x\""]);

    let (result, _) = run(&[install(10, &path)]);

    assert_eq!(
        result,
        Err(format!(
            "Bad library \"{}\": Line 2 has no line number: PRINT \"x\"",
            path
        ))
    );
}

#[test]
fn a_read_in_a_library_routine_reads_the_data_of_the_program() {
    let dir = test_dir("data");
    let path = write_library(
        &dir,
        "datalib.bbas",
        &[
            "10 DEF PROCr",
            "20 READ A",
            "30 PRINT ;A",
            "40 ENDPROC",
            "50 DATA 99",
        ],
    );
    let mut source = vec![install(10, &path)];
    source.extend(lines(&["20 PROCr", "30 END", "40 DATA 5"]));

    let (result, output) = run(&source);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "5\n");
}

#[test]
fn a_variable_and_an_array_named_install_still_work() {
    let source = lines(&[
        "10 INSTALL = 3",
        "20 PRINT ;INSTALL",
        "30 DIM INSTALL(3)",
        "40 INSTALL(2) = 7",
        "50 PRINT ;INSTALL(2)",
    ]);

    let (result, output) = run(&source);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "3\n7\n");
}

#[test]
fn run_removes_the_libraries_and_a_second_run_installs_them_again() {
    let dir = test_dir("second_run");
    write_library(
        &dir,
        "runlib.bbas",
        &["10 DEF PROCa", "20 PRINT \"lib a\"", "30 ENDPROC"],
    );

    let (status, output) = run_in_dir(
        &dir,
        &["10 INSTALL \"runlib\"", "20 PROCa", "RUN", "RUN", "EXIT"],
    );

    assert!(status.success());
    assert_eq!(output.matches("lib a").count(), 2, "output: {}", output);
}

#[test]
fn clear_keeps_the_libraries_and_new_and_load_remove_them() {
    let dir = test_dir("clear_new_load");
    write_library(
        &dir,
        "keeplib.bbas",
        &["10 DEF PROCk", "20 PRINT \"kept\"", "30 ENDPROC"],
    );

    let (_, cleared) = run_in_dir(
        &dir,
        &["10 INSTALL \"keeplib\"", "RUN", "CLEAR", "PROCk", "EXIT"],
    );
    let (_, after_new) = run_in_dir(
        &dir,
        &["10 INSTALL \"keeplib\"", "RUN", "NEW", "PROCk", "EXIT"],
    );
    let (_, after_load) = run_in_dir(
        &dir,
        &[
            "10 INSTALL \"keeplib\"",
            "SAVE \"prog\"",
            "RUN",
            "LOAD \"prog\"",
            "PROCk",
            "EXIT",
        ],
    );

    assert!(cleared.contains("kept"), "output: {}", cleared);
    assert!(
        after_new.contains("No such FN/PROC"),
        "output: {}",
        after_new
    );
    assert!(
        after_load.contains("No such FN/PROC"),
        "output: {}",
        after_load
    );
}

#[test]
fn list_and_save_do_not_show_a_library() {
    let dir = test_dir("list_save");
    write_library(
        &dir,
        "hidelib.bbas",
        &["10 DEF PROChidden", "20 PRINT \"x\"", "30 ENDPROC"],
    );

    let (_, output) = run_in_dir(
        &dir,
        &[
            "10 INSTALL \"hidelib\"",
            "RUN",
            "LIST",
            "SAVE \"shown\"",
            "EXIT",
        ],
    );
    let saved = fs::read_to_string(dir.join("shown.bbas")).expect("program is saved");

    assert!(output.contains("10 INSTALL"), "output: {}", output);
    assert!(!output.contains("PROChidden"), "output: {}", output);
    assert_eq!(saved, "10 INSTALL \"hidelib\"\n");
}

#[test]
fn the_binary_runs_a_program_with_a_library() {
    let dir = test_dir("binary");
    let path = write_library(
        &dir,
        "binlib.bbas",
        &[
            "10 DEF PROCgreet(N$)",
            "20 PRINT \"Hello \";N$",
            "30 ENDPROC",
        ],
    );

    let output = run_binary_read_first(&format!(
        "10 INSTALL \"{}\"\n20 PROCgreet(\"Bob\")\nRUN\nEXIT\n",
        path
    ));

    assert!(output.contains("Hello Bob"), "output: {}", output);
}
