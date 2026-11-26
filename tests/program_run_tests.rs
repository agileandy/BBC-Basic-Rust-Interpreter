//! Tests that run complete BASIC programs through the library run loop.

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;

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

#[test]
fn test_for_loop_runs_body_for_each_value() {
    let (result, output) = run(&[
        "10 FOR I% = 1 TO 3",
        "20 PRINT \"X\"",
        "30 NEXT I%",
        "40 PRINT \"DONE\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "X\nX\nX\nDONE\n");
}

#[test]
fn test_empty_program_gives_error() {
    let (result, output) = run(&[]);

    assert_eq!(result, Err("No program to run".to_string()));
    assert_eq!(output, "");
}

#[test]
fn test_goto_skips_lines() {
    let (result, output) = run(&["10 GOTO 30", "20 PRINT \"SKIPPED\"", "30 PRINT \"TARGET\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "TARGET\n");
}

#[test]
fn test_goto_missing_line_gives_error() {
    let (result, _) = run(&["10 GOTO 99"]);

    assert_eq!(result, Err("Line 99 not found (GOTO)".to_string()));
}

#[test]
fn test_gosub_returns_to_line_after_call() {
    let (result, output) = run(&[
        "10 GOSUB 100",
        "20 PRINT \"BACK\"",
        "30 END",
        "100 PRINT \"SUB\"",
        "110 RETURN",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "SUB\nBACK\n");
}

#[test]
fn test_repeat_until_runs_until_condition_is_true() {
    let (result, output) = run(&[
        "10 N% = 0",
        "20 REPEAT",
        "30 PRINT \"R\"",
        "40 N% = N% + 1",
        "50 UNTIL N% = 2",
        "60 PRINT \"DONE\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "R\nR\nDONE\n");
}

#[test]
fn test_while_loops_while_condition_is_true() {
    let (result, output) = run(&[
        "10 N% = 0",
        "20 WHILE N% < 2",
        "30 PRINT \"W\"",
        "40 N% = N% + 1",
        "50 ENDWHILE",
        "60 PRINT \"DONE\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "W\nW\nDONE\n");
}

#[test]
fn test_false_while_skips_nested_loop_body() {
    let (result, output) = run(&[
        "10 N% = 9",
        "20 WHILE N% < 2",
        "30 WHILE N% < 1",
        "40 PRINT \"INNER\"",
        "50 ENDWHILE",
        "60 PRINT \"OUTER\"",
        "70 ENDWHILE",
        "80 PRINT \"DONE\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "DONE\n");
}

#[test]
fn test_procedure_receives_arguments() {
    let (result, output) = run(&[
        "10 PROC show(\"A\", \"B\")",
        "20 PRINT \"BACK\"",
        "30 END",
        "100 DEF PROC show(P$, Q$)",
        "110 PRINT Q$; P$",
        "120 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "BA\nBACK\n");
}

#[test]
fn test_on_error_jumps_to_handler() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 X = 1 / 0",
        "30 PRINT \"NOT REACHED\"",
        "40 END",
        "100 PRINT \"CAUGHT\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "CAUGHT\n");
}

#[test]
fn test_error_without_handler_stops_program() {
    let (result, output) = run(&["10 PRINT \"START\"", "20 X = 1 / 0", "30 PRINT \"END\""]);

    assert!(
        result.is_err(),
        "expected a runtime error, got {:?}",
        result
    );
    assert_eq!(output, "START\n");
}

/// Examples that the interpreter cannot run yet. Remove a name when its defect is corrected.
const KNOWN_FAILING_EXAMPLES: &[&str] = &[
    "sinewave.bas",            // does not terminate
    "test_error_handling.bas", // PRINT of ERR fails in the handler, which then loops
    "test_def_fn.bas",         // parse error: "Expected = after function parameters"
    "test_functions.bas",      // PI is not known as a variable
    "test_proc.bas",           // parse error: "Invalid assignment"
    "test_while.bas",          // TypeMismatch at line 550
];

#[test]
fn test_each_example_program_runs_without_error() {
    let examples = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut failures = Vec::new();
    let mut count = 0;

    for entry in std::fs::read_dir(&examples).expect("examples directory exists") {
        let path = entry.expect("directory entry is readable").path();
        if path.extension().and_then(|e| e.to_str()) != Some("bas") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if KNOWN_FAILING_EXAMPLES.contains(&name.as_str()) {
            continue;
        }
        count += 1;
        let source = std::fs::read_to_string(&path).expect("example is readable");
        // Only numbered lines are program lines
        let lines: Vec<&str> = source
            .lines()
            .filter(|line| line.trim_start().starts_with(|c: char| c.is_ascii_digit()))
            .collect();
        match std::panic::catch_unwind(|| run(&lines).0) {
            Ok(Ok(())) => {}
            Ok(Err(e)) => failures.push(format!("{}: {}", name, e)),
            Err(_) => failures.push(format!("{}: panic", name)),
        }
    }

    assert!(count > 0, "no example programs found");
    assert!(
        failures.is_empty(),
        "examples failed:\n{}",
        failures.join("\n")
    );
}

#[test]
fn test_data_is_read_before_its_line_runs() {
    let (result, output) = run(&["10 READ A$", "20 PRINT A$", "30 DATA \"HELLO\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "HELLO\n");
}
