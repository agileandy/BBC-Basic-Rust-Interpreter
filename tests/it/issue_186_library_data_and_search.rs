//! Issue #186: a library reads its own DATA, a library routine installs a
//! second library, INSTALL searches 3 places for a file, and 2 example
//! libraries. The reference is the BBC BASIC for Windows manual: RESTORE
//! (bbcwin7.html#restore), LOCAL DATA (bbcwin5.html#localdata) and INSTALL
//! (bbcwin5.html#install). Each test writes its files to its own directory.

use crate::common::run_with_limit;
use bbc_basic_interpreter::commands::run_command;
use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::library::{library_directories_from, LIBRARY_PATH_VARIABLE};
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;
use bbc_basic_interpreter::ui::App;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// A new empty directory with a name that only this test uses.
fn test_dir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bbc_issue186_{}_{}", test, std::process::id()));
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

/// The program line `<number> INSTALL "<path>"`.
fn install(number: u16, path: &str) -> String {
    format!("{} INSTALL \"{}\"", number, path)
}

/// A library with no line numbers. FNsum adds the 3 items of its own
/// DATA table, and keeps the DATA pointer of its caller.
const SUM_LIBRARY: [&str; 11] = [
    "DEF FNsum",
    "LOCAL DATA",
    "LOCAL T, I, V",
    "RESTORE +1",
    "DATA 1, 2, 3",
    "FOR I = 1 TO 3",
    "READ V",
    "T = T + V",
    "NEXT",
    "=T",
    "DEF PROCnothing : ENDPROC",
];

/// Run the program lines with the time limit of `run_with_limit`.
fn run(source: &[String]) -> (Result<(), String>, String) {
    let lines: Vec<&str> = source.iter().map(String::as_str).collect();
    run_with_limit(&lines)
}

fn lines(source: &[&str]) -> Vec<String> {
    source.iter().map(|line| line.to_string()).collect()
}

#[test]
fn restore_plus_n_goes_to_the_data_n_lines_after_the_restore() {
    let (result, output) = run(&lines(&[
        "10 DATA 1",
        "20 RESTORE +2",
        "30 DATA 2",
        "40 DATA 3",
        "50 READ A",
        "60 PRINT ;A",
    ]));

    assert_eq!(result, Ok(()));
    assert_eq!(output, "3\n");
}

#[test]
fn restore_data_restores_the_pointer_that_local_data_saved() {
    let (result, output) = run(&lines(&[
        "10 DATA 1, 2",
        "20 READ A",
        "30 LOCAL DATA : RESTORE +2",
        "40 PRINT \"skip\"",
        "50 DATA 7",
        "60 READ B",
        "70 RESTORE DATA",
        "80 READ C",
        "90 PRINT ;A;\" \";B;\" \";C",
    ]));

    assert_eq!(result, Ok(()));
    assert_eq!(output, "skip\n1 7 2\n");
}

#[test]
fn restore_data_with_no_saved_pointer_is_not_local() {
    let (result, output) = run(&lines(&["10 DATA 1", "20 RESTORE DATA"]));

    assert_eq!(result, Err("Not LOCAL at line 20".to_string()));
    assert_eq!(output, "");
}

#[test]
fn a_data_line_keeps_its_text_after_a_colon() {
    let (result, output) = run(&lines(&[
        "10 DATA A:B, C",
        "20 READ X$, Y$",
        "30 PRINT X$;\"/\";Y$",
    ]));

    assert_eq!(result, Ok(()));
    assert_eq!(output, "A:B/C\n");
}

#[test]
fn a_library_routine_reads_its_own_data_two_times() {
    let dir = test_dir("own_data");
    let path = write_library(&dir, "sumlib.bbas", &SUM_LIBRARY);
    let mut source = vec![install(10, &path)];
    source.extend(lines(&["20 PRINT ;FNsum", "30 PRINT ;FNsum"]));

    let (result, output) = run(&source);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "6\n6\n");
}

#[test]
fn the_data_pointer_of_the_program_is_the_same_after_a_library_routine() {
    let dir = test_dir("program_pointer");
    let path = write_library(&dir, "sumlib.bbas", &SUM_LIBRARY);
    let mut source = vec![install(10, &path)];
    source.extend(lines(&[
        "20 DATA 10, 20",
        "30 READ A",
        "40 PRINT ;FNsum",
        "50 READ B",
        "60 PRINT ;A;\" \";B",
    ]));

    let (result, output) = run(&source);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "6\n10 20\n");
}

#[test]
fn a_read_in_the_program_does_not_read_the_data_of_a_library() {
    let dir = test_dir("no_spill");
    let path = write_library(&dir, "sumlib.bbas", &SUM_LIBRARY);
    let mut source = vec![install(10, &path)];
    source.extend(lines(&["20 DATA 5", "30 READ A, B"]));

    let (result, output) = run(&source);

    assert_eq!(result, Err("Out of DATA at line 30".to_string()));
    assert_eq!(output, "");
}

#[test]
fn a_library_routine_installs_a_second_library() {
    let dir = test_dir("nested");
    let second = write_library(
        &dir,
        "second.bbas",
        &["DEF PROCb", "PRINT \"b\"", "ENDPROC"],
    );
    let first = write_library(
        &dir,
        "first.bbas",
        &[
            "DEF PROCa",
            &format!("INSTALL \"{}\"", second),
            "PROCb",
            "ENDPROC",
        ],
    );
    let mut source = vec![install(10, &first)];
    source.extend(lines(&["20 PROCa", "30 PROCb"]));

    let (result, output) = run(&source);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "b\nb\n");
}

#[test]
fn two_libraries_that_install_each_other_do_not_loop_and_load_one_time() {
    let dir = test_dir("cycle");
    let first_path = dir.join("cyclea.bbas").to_string_lossy().into_owned();
    let second_path = dir.join("cycleb.bbas").to_string_lossy().into_owned();
    let first_install = format!("INSTALL \"{}\"", second_path);
    let second_install = format!("INSTALL \"{}\"", first_path);
    write_library(
        &dir,
        "cyclea.bbas",
        &["DEF PROCa", &first_install, "PRINT \"a\"", "ENDPROC"],
    );
    write_library(
        &dir,
        "cycleb.bbas",
        &["DEF PROCb", &second_install, "PRINT \"b\"", "ENDPROC"],
    );
    let source = vec![
        install(10, &first_path),
        "20 PROCa".to_string(),
        "30 PROCb".to_string(),
        "40 PROCa".to_string(),
    ];

    let (result, output, count) = with_time_limit(move || {
        let mut executor = Executor::new();
        let mut program = ProgramStore::new();
        for line in &source {
            program.store_line(tokenize(line).expect("line tokenizes"));
        }
        let result = run_program(&mut executor, &mut program);
        let count = executor.libraries().libraries().len();
        (result, executor.get_output().to_string(), count)
    });

    assert_eq!(result, Ok(()));
    assert_eq!(output, "a\nb\na\n");
    assert_eq!(count, 2);
}

/// Run each typed line with `run_command`, as at the prompt, with a time
/// limit. Give the result of the last line and the output.
fn run_commands(commands: Vec<String>) -> (Result<(), String>, String) {
    run_commands_with(Vec::new(), commands)
}

/// `run_commands` with these library directories in place of those of
/// `BBCBASIC_LIB`
fn run_commands_with(
    library_directories: Vec<PathBuf>,
    commands: Vec<String>,
) -> (Result<(), String>, String) {
    with_time_limit(move || {
        let mut executor = Executor::new();
        executor
            .libraries_mut()
            .set_library_directories(library_directories);
        let mut program = ProgramStore::new();
        let mut result = Ok(());
        for command in &commands {
            result = run_command(&mut executor, &mut program, command).map(|_| ());
        }
        (result, executor.get_output().to_string())
    })
}

#[test]
fn install_finds_a_library_in_the_directory_of_the_program_file() {
    let dir = test_dir("program_dir");
    write_library(
        &dir,
        "pdlib.bbas",
        &["DEF PROCp", "PRINT \"found\"", "ENDPROC"],
    );
    let program = write_library(&dir, "pdprog.bbas", &["10 INSTALL \"pdlib\"", "20 PROCp"]);

    let (result, output) = run_commands(vec![format!("LOAD \"{}\"", program), "RUN".into()]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, format!("Loaded from {}\nfound\n", program));
}

#[test]
fn install_finds_a_library_in_the_directory_of_a_saved_program() {
    let dir = test_dir("saved_dir");
    write_library(
        &dir,
        "svlib.bbas",
        &["DEF PROCs", "PRINT \"saved\"", "ENDPROC"],
    );
    let program = dir.join("svprog.bbas").to_string_lossy().into_owned();
    let commands = vec![
        "10 INSTALL \"svlib\"".to_string(),
        "20 PROCs".to_string(),
        format!("SAVE \"{}\"", program),
        "RUN".to_string(),
    ];

    let (result, output) = run_commands(commands);

    assert_eq!(result, Ok(()));
    assert_eq!(output, format!("Saved to {}\nsaved\n", program));
}

#[test]
fn install_searches_each_library_directory_in_order() {
    let first = test_dir("lib_dir_first");
    let second = test_dir("lib_dir_second");
    write_library(
        &second,
        "ldlib.bbas",
        &["DEF PROCl", "PRINT \"second\"", "ENDPROC"],
    );
    let commands = lines(&["10 INSTALL \"ldlib\"", "20 PROCl", "RUN"]);

    let (result, output) = run_commands_with(vec![first, second], commands);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "second\n");
}

#[test]
fn the_directory_of_the_program_comes_before_a_library_directory() {
    let program_dir = test_dir("order_program");
    let library_dir = test_dir("order_library");
    write_library(
        &program_dir,
        "orlib.bbas",
        &["DEF PROCo", "PRINT \"program\"", "ENDPROC"],
    );
    write_library(
        &library_dir,
        "orlib.bbas",
        &["DEF PROCo", "PRINT \"library\"", "ENDPROC"],
    );
    let program = write_library(
        &program_dir,
        "orprog.bbas",
        &["10 INSTALL \"orlib\"", "20 PROCo"],
    );
    let commands = vec![format!("LOAD \"{}\"", program), "RUN".to_string()];

    let (result, output) = run_commands_with(vec![library_dir], commands);

    assert_eq!(result, Ok(()));
    assert_eq!(output, format!("Loaded from {}\nprogram\n", program));
}

#[test]
fn a_name_with_a_directory_part_is_not_searched() {
    let library_dir = test_dir("directory_part");
    fs::create_dir_all(library_dir.join("sub")).expect("sub directory is made");
    write_library(
        &library_dir.join("sub"),
        "dplib.bbas",
        &["DEF PROCd", "ENDPROC"],
    );
    let commands = lines(&["10 INSTALL \"sub/dplib\"", "RUN"]);

    let (result, output) = run_commands_with(vec![library_dir], commands);

    assert_eq!(
        result,
        Err("File not found: sub/dplib.bbas at line 10".to_string())
    );
    assert_eq!(output, "");
}

#[test]
fn the_value_of_bbcbasic_lib_is_a_list_of_directories() {
    assert_eq!(
        library_directories_from("/a/lib::b:"),
        [PathBuf::from("/a/lib"), PathBuf::from("b")]
    );
    assert_eq!(library_directories_from(""), Vec::<PathBuf>::new());
}

/// The path of an example library of `examples/lib`
fn example_library(file: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/lib")
        .join(file)
        .to_string_lossy()
        .into_owned()
}

/// Install the example library and run the program lines after it
fn run_with_example(file: &str, source: &[&str]) -> (Result<(), String>, String) {
    let mut program = vec![install(1, &example_library(file))];
    program.extend(lines(source));
    run(&program)
}

#[test]
fn fntrim_removes_the_spaces_at_the_start_and_the_end() {
    let (result, output) = run_with_example(
        "strings.bbas",
        &[
            "10 PRINT \"[\";FNtrim$(\"  two words  \");\"]\"",
            "20 PRINT \"[\";FNtrim$(\"   \");\"]\"",
            "30 PRINT \"[\";FNtrim$(\"none\");\"]\"",
        ],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "[two words]\n[]\n[none]\n");
}

#[test]
fn fnpadleft_and_fnpadright_add_spaces_to_a_length() {
    let (result, output) = run_with_example(
        "strings.bbas",
        &[
            "10 PRINT \"[\";FNpadleft$(\"ab\", 5);\"]\"",
            "20 PRINT \"[\";FNpadright$(\"ab\", 5);\"]\"",
            "30 PRINT \"[\";FNpadleft$(\"abcdef\", 3);\"]\"",
            "40 PRINT \"[\";FNpadright$(\"abcdef\", 3);\"]\"",
        ],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "[   ab]\n[ab   ]\n[abcdef]\n[abcdef]\n");
}

#[test]
fn fnrepeat_repeats_a_string() {
    let (result, output) = run_with_example(
        "strings.bbas",
        &[
            "10 PRINT \"[\";FNrepeat$(\"ab\", 3);\"]\"",
            "20 PRINT \"[\";FNrepeat$(\"ab\", 0);\"]\"",
        ],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "[ababab]\n[]\n");
}

#[test]
fn fnsplit_gives_a_list_of_the_parts() {
    let (result, output) = run_with_example(
        "strings.bbas",
        &[
            "10 L% = FNsplit(\"red,green,,blue\", \",\")",
            "20 PRINT ;LISTLEN(L%)",
            "30 FOR I% = 0 TO LISTLEN(L%) - 1",
            "40 PRINT \"[\";LISTGET$(L%, I%);\"]\"",
            "50 NEXT",
            "60 M% = FNsplit(\"a--b\", \"--\")",
            "70 PRINT LISTGET$(M%, 0);\" \";LISTGET$(M%, 1)",
            "80 N% = FNsplit(\"whole\", \"\")",
            "90 PRINT ;LISTLEN(N%);\" \";LISTGET$(N%, 0)",
        ],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "4\n[red]\n[green]\n[]\n[blue]\na b\n1 whole\n");
}

#[test]
fn fnmin_and_fnmax_give_the_smaller_and_the_larger_value() {
    let (result, output) = run_with_example(
        "numbers.bbas",
        &[
            "10 PRINT ;FNmin(3, 7);\" \";FNmin(7, 3);\" \";FNmin(-2.5, 1)",
            "20 PRINT ;FNmax(3, 7);\" \";FNmax(7, 3);\" \";FNmax(-2.5, 1)",
        ],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "3 3 -2.5\n7 7 1\n");
}

#[test]
fn fnclamp_keeps_a_value_in_a_range() {
    let (result, output) = run_with_example(
        "numbers.bbas",
        &["10 PRINT ;FNclamp(5, 0, 10);\" \";FNclamp(-4, 0, 10);\" \";FNclamp(12, 0, 10)"],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "5 0 10\n");
}

#[test]
fn fnround_rounds_to_a_count_of_decimal_places() {
    let (result, output) = run_with_example(
        "numbers.bbas",
        &[
            "10 PRINT ;FNround(3.14159, 2);\" \";FNround(1234.5678, 1)",
            "20 PRINT ;FNround(2.5, 0);\" \";FNround(-2.5, 0);\" \";FNround(-1.26, 1)",
        ],
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, "3.14 1234.6\n3 -3 -1.3\n");
}

#[test]
fn the_library_demo_prints_a_result_of_each_routine() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    let output = run_binary_in(
        root,
        None,
        "LOAD \"examples/library_demo.bas\"\nRUN\nEXIT\n",
    );

    let expected = [
        "Strings",
        "[trim me]",
        "[   42]",
        "[42   ]",
        "-=-=-=",
        "3 parts: one two three",
        "Numbers",
        "min 2 max 9",
        "clamp 100",
        "round 3.142",
    ]
    .join("\n");
    assert!(output.contains(&format!("> {}\n> ", expected)), "{output}");
}

/// The text of each row of the IDE screen
fn ide_rows(app: &App) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("test terminal");
    terminal.draw(|frame| app.draw(frame)).expect("draw");
    let buffer = terminal.backend().buffer().clone();
    (0..buffer.area.height)
        .map(|row| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, row)].symbol())
                .collect()
        })
        .collect()
}

#[test]
fn the_ide_searches_the_directory_of_the_document_file() {
    let dir = test_dir("ide_document");
    write_library(
        &dir,
        "idelib.bbas",
        &["DEF PROCi", "PRINT \"ide found\"", "ENDPROC"],
    );
    let program = write_library(&dir, "ideprog.bbas", &["10 INSTALL \"idelib\"", "20 PROCi"]);
    let mut app = App::new(Some(program));

    app.key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    assert!(
        app.wait_for_run(Duration::from_secs(5)),
        "the run did not end"
    );

    let rows = ide_rows(&app);
    assert!(
        rows.iter().any(|row| row.trim_end() == "ide found"),
        "{rows:#?}"
    );
}

/// Run the binary in `dir` with `BBCBASIC_LIB` set to `library_path` (or
/// not set), write the input, and give stdout. Kill it after 5 seconds.
fn run_binary_in(dir: &Path, library_path: Option<&str>, input: &str) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_bbc-basic-interpreter"));
    command
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    match library_path {
        Some(value) => command.env(LIBRARY_PATH_VARIABLE, value),
        None => command.env_remove(LIBRARY_PATH_VARIABLE),
    };
    let mut child = command.spawn().expect("binary starts");
    let mut stdout = child.stdout.take().expect("stdout is piped");
    let reader = std::thread::spawn(move || {
        let mut output = String::new();
        let _ = stdout.read_to_string(&mut output);
        output
    });
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(input.as_bytes())
        .expect("input is written");
    let deadline = Instant::now() + Duration::from_secs(5);
    while child
        .try_wait()
        .expect("child status is readable")
        .is_none()
    {
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("interpreter did not stop within 5 seconds");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    reader.join().expect("reader thread completes")
}

#[test]
fn install_finds_a_library_in_the_working_directory() {
    let dir = test_dir("working_dir");
    write_library(
        &dir,
        "wdlib.bbas",
        &["DEF PROCw", "PRINT \"working\"", "ENDPROC"],
    );

    let output = run_binary_in(&dir, None, "10 INSTALL \"wdlib\"\n20 PROCw\nRUN\nEXIT\n");

    assert!(output.contains("> working\n"), "{output}");
}

#[test]
fn install_finds_a_library_in_a_directory_of_bbcbasic_lib() {
    let dir = test_dir("env_dir");
    let library_dir = test_dir("env_library");
    write_library(
        &library_dir,
        "evlib.bbas",
        &["DEF PROCv", "PRINT \"env\"", "ENDPROC"],
    );
    let value = format!("/no/such/dir:{}", library_dir.display());

    let output = run_binary_in(
        &dir,
        Some(&value),
        "10 INSTALL \"evlib\"\n20 PROCv\nRUN\nEXIT\n",
    );

    assert!(output.contains("> env\n"), "{output}");
}

#[test]
fn the_working_directory_comes_before_a_directory_of_bbcbasic_lib() {
    let dir = test_dir("order_working");
    let library_dir = test_dir("order_env");
    write_library(
        &dir,
        "owlib.bbas",
        &["DEF PROCw", "PRINT \"working\"", "ENDPROC"],
    );
    write_library(
        &library_dir,
        "owlib.bbas",
        &["DEF PROCw", "PRINT \"env\"", "ENDPROC"],
    );
    let value = library_dir.to_string_lossy().into_owned();

    let output = run_binary_in(
        &dir,
        Some(&value),
        "10 INSTALL \"owlib\"\n20 PROCw\nRUN\nEXIT\n",
    );

    assert!(output.contains("> working\n"), "{output}");
    assert!(!output.contains("env"), "{output}");
}

/// Do `work` on a thread, and panic if it takes more than 5 seconds
fn with_time_limit<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(work());
    });
    receiver
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("the work ends within 5 seconds and does not panic")
}
