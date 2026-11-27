//! Tests that a PROC parameter is local to its procedure: the variable of
//! the caller with the same name gets its old value back at ENDPROC
//! (issue #108).

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;

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

#[test]
fn test_real_parameter_restores_caller_variable() {
    let (result, output) = run(&[
        "10 X = 7 : PROCa(1)",
        "20 PRINT X",
        "30 END",
        "40 DEF PROCa(X)",
        "50 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "7\n");
}

#[test]
fn test_recursive_procedure_keeps_value_of_each_level() {
    let (result, output) = run(&[
        "10 PROCcount(3)",
        "20 END",
        "100 DEF PROCcount(N)",
        "110 PRINT N",
        "120 IF N > 1 THEN PROCcount(N - 1)",
        "130 PRINT N",
        "140 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "3\n2\n1\n1\n2\n3\n");
}

#[test]
fn test_string_parameter_restores_caller_variable() {
    let (result, output) = run(&[
        "10 S$ = \"OLD\" : PROCa(\"NEW\")",
        "20 PRINT S$",
        "30 END",
        "40 DEF PROCa(S$)",
        "50 PRINT S$",
        "60 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "NEW\nOLD\n");
}

#[test]
fn test_integer_parameter_restores_caller_variable() {
    let (result, output) = run(&[
        "10 N% = 7 : PROCa(1)",
        "20 PRINT N%",
        "30 END",
        "40 DEF PROCa(N%)",
        "50 PRINT N%",
        "60 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n7\n");
}

#[test]
fn test_resident_integer_parameter_restores_caller_variable() {
    let (result, output) = run(&[
        "10 A% = 7 : PROCa(1)",
        "20 PRINT A%",
        "30 END",
        "40 DEF PROCa(A%)",
        "50 PRINT A%",
        "60 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n7\n");
}

#[test]
fn test_second_argument_uses_caller_value_of_first_name() {
    let (result, output) = run(&[
        "10 X = 5 : PROCa(1, X)",
        "20 PRINT X",
        "30 END",
        "40 DEF PROCa(X, Y)",
        "50 PRINT X; \" \"; Y",
        "60 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1 5\n5\n");
}

#[test]
fn test_error_in_second_argument_leaves_first_variable_unchanged() {
    let (result, output) = run(&[
        "10 X = 7",
        "20 ON ERROR GOTO 100",
        "30 PROCa(1, 1 / 0)",
        "40 END",
        "50 DEF PROCa(X, Y)",
        "60 ENDPROC",
        "100 PRINT X",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "7\n");

    let (result, _) = run(&[
        "10 X = 7",
        "20 PROCa(1, 1 / 0)",
        "30 END",
        "40 DEF PROCa(X, Y)",
        "50 ENDPROC",
    ]);
    assert_eq!(result, Err("Division by zero at line 20".to_string()));
}

#[test]
fn test_local_of_parameter_name_restores_caller_variable() {
    let (result, output) = run(&[
        "10 X = 7 : PROCa(1)",
        "20 PRINT X",
        "30 END",
        "40 DEF PROCa(X)",
        "50 LOCAL X",
        "60 X = 3",
        "70 ENDPROC",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "7\n");
}
