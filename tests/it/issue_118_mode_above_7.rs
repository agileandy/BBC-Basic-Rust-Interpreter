//! Issue #118: a MODE number from 8 to 255 selects mode `n AND 7`, as on a
//! BBC Micro Model B. A number below 0 or above 255 gives Bad MODE.

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::graphics::ScreenMode;
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::tokenizer::tokenize;

/// Store the source lines, run the program and return the result and the
/// executor, so that a test can read the screen mode.
fn run_executor(source: &[&str]) -> (Result<(), String>, Executor) {
    let mut executor = Executor::new();
    let mut program = ProgramStore::new();
    for line in source {
        program.store_line(tokenize(line).expect("line tokenizes"));
    }
    let result = run_program(&mut executor, &mut program);
    (result, executor)
}

fn mode(number: i32) -> ScreenMode {
    ScreenMode::from_number(number).expect("mode is in the mode table")
}

#[test]
fn test_mode_8_selects_mode_0() {
    let (result, executor) = run_executor(&["10 MODE 1", "20 MODE 8"]);

    assert_eq!(result, Ok(()));
    assert_eq!(executor.screen_mode(), mode(0));
}

#[test]
fn test_mode_12_selects_mode_4() {
    let (result, executor) = run_executor(&["10 MODE 12"]);

    assert_eq!(result, Ok(()));
    assert_eq!(executor.screen_mode(), mode(4));
}

#[test]
fn test_mode_255_selects_mode_7() {
    let (result, executor) = run_executor(&["10 MODE 255"]);

    assert_eq!(result, Ok(()));
    assert_eq!(executor.screen_mode(), mode(7));
}

#[test]
fn test_mode_256_gives_bad_mode() {
    let (result, executor) = run_executor(&["10 MODE 256"]);

    assert_eq!(result, Err("Bad MODE at line 10".to_string()));
    assert_eq!(executor.get_output(), "");
}

#[test]
fn test_mode_minus_1_gives_bad_mode() {
    let (result, _) = run_executor(&["10 MODE -1"]);

    assert_eq!(result, Err("Bad MODE at line 10".to_string()));
}

#[test]
fn test_vdu_22_9_selects_mode_1() {
    let (result, executor) = run_executor(&["10 VDU 22, 9"]);

    assert_eq!(result, Ok(()));
    assert_eq!(executor.screen_mode(), mode(1));
}
