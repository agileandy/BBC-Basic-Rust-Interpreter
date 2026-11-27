//! Issue 24: STEREO is a statement with no effect, and ADVAL gives 0.

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::{detokenize, tokenize};

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
fn test_adval_of_analogue_channel_gives_zero() {
    let (result, output) = run(&["10 PRINT ADVAL(1)"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_adval_of_fire_buttons_and_buffer_gives_zero() {
    let (result, output) = run(&["10 PRINT ADVAL(0)", "20 PRINT ADVAL(-1)"]);

    assert_eq!(result, Ok(()));
    let values: Vec<&str> = output.lines().map(str::trim).collect();
    assert_eq!(values, ["0", "0"]);
}

#[test]
fn test_adval_is_an_integer_in_an_expression() {
    let (result, output) = run(&["10 X% = ADVAL(1) + 1", "20 PRINT X%"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_adval_with_wrong_argument_count_gives_error() {
    for source in ["10 PRINT ADVAL()", "10 PRINT ADVAL(1, 2)"] {
        let (result, _) = run(&[source]);

        assert!(result.is_err(), "{source} gave {result:?}");
    }
}

#[test]
fn test_stereo_runs_with_no_effect() {
    let (result, output) = run(&["10 STEREO 1, -50 : PRINT \"OK\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "OK\n");
}

#[test]
fn test_stereo_evaluates_its_arguments() {
    let (result, _) = run(&["10 STEREO 1, NOSUCH%"]);

    assert!(
        result.is_err(),
        "STEREO with an unknown variable gave {result:?}"
    );
}

#[test]
fn test_stereo_with_wrong_argument_count_gives_error_at_its_line() {
    let (result, _) = run(&["10 PRINT \"A\"", "20 STEREO 1"]);

    let message = result.expect_err("STEREO 1 is an error");
    assert!(message.contains("at line 20"), "message: {message}");
}

/// LIST writes no space after a comma, for each statement. So the listed
/// form of `STEREO 1, -50` is `STEREO 1,-50`.
#[test]
fn test_list_shows_stereo_and_adval_as_typed() {
    for (source, expected) in [
        ("10 STEREO 1,-50", "10 STEREO 1,-50"),
        ("20 X% = ADVAL(1)", "20 X% = ADVAL(1)"),
    ] {
        let line = tokenize(source).unwrap();
        let listed = detokenize(&line).unwrap();

        assert_eq!(listed, expected);
        assert_eq!(tokenize(&listed).unwrap(), line);
    }
}
