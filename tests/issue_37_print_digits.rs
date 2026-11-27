//! Tests for issue 37: PRINT shows a maximum of 9 significant digits.

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
fn test_print_tan_of_45_degrees_shows_one() {
    assert_eq!(run(&["10 PRINT TAN(RAD(45))"]), (Ok(()), "1\n".to_string()));
}

#[test]
fn test_print_sin_of_30_degrees_times_ten_shows_five() {
    assert_eq!(
        run(&["10 PRINT SIN(RAD(30)) * 10"]),
        (Ok(()), "5\n".to_string())
    );
}

#[test]
fn test_print_one_third_shows_nine_digits() {
    assert_eq!(
        run(&["10 PRINT 1 / 3"]),
        (Ok(()), "0.333333333\n".to_string())
    );
}

#[test]
fn test_print_seven_halves_shows_three_point_five() {
    assert_eq!(run(&["10 PRINT 7 / 2"]), (Ok(()), "3.5\n".to_string()));
}

#[test]
fn test_print_str_of_one_third_shows_nine_digits() {
    assert_eq!(
        run(&["10 PRINT STR$(1 / 3)"]),
        (Ok(()), "0.333333333\n".to_string())
    );
}

#[test]
fn test_print_integer_sum_shows_integer() {
    assert_eq!(run(&["10 PRINT 2 + 3"]), (Ok(()), "5\n".to_string()));
}

#[test]
fn test_print_real_variable_shows_nine_digits() {
    assert_eq!(
        run(&["10 A = 2 / 3", "20 PRINT A"]),
        (Ok(()), "0.666666667\n".to_string())
    );
}

#[test]
fn test_print_large_real_uses_exponent_form() {
    assert_eq!(
        run(&["10 PRINT 2.5 * 1000000000"]),
        (Ok(()), "2.5E9\n".to_string())
    );
}

#[test]
fn test_print_large_integer_variable_keeps_integer_form() {
    assert_eq!(
        run(&["10 A% = 1234567890", "20 PRINT A%"]),
        (Ok(()), "1234567890\n".to_string())
    );
}
