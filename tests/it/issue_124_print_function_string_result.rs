//! Issue #124: PRINT of a function with no `$` in its name shows the type of
//! its result. A string result prints as text, and the body runs one time.

use crate::common::run_with_limit as run;

#[test]
fn test_print_single_line_function_with_string_result() {
    let (result, output) = run(&["10 PRINT FNt", "20 END", "30 DEF FNt = \"one line\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "one line\n");
}

#[test]
fn test_print_multi_line_function_with_string_result() {
    let (result, output) = run(&[
        "10 PRINT FNt",
        "20 END",
        "30 DEF FNt",
        "40 = \"many lines\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "many lines\n");
}

#[test]
fn test_print_string_function_runs_its_body_one_time() {
    let (result, output) = run(&[
        "10 C% = 0",
        "20 PRINT FNt",
        "30 PRINT C%",
        "40 END",
        "50 DEF FNt",
        "60 C% = C% + 1",
        "70 = \"counted\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "counted\n1\n");
}

#[test]
fn test_print_real_function_runs_its_body_one_time() {
    let (result, output) = run(&[
        "10 C% = 0",
        "20 PRINT FNr",
        "30 PRINT C%",
        "40 END",
        "50 DEF FNr",
        "60 C% = C% + 1",
        "70 = 1.5",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1.5\n1\n");
}

#[test]
fn test_print_mixed_function_result_types_on_one_line() {
    let (result, output) = run(&[
        "10 PRINT FNa; FNb$; FNc%; FNt",
        "20 END",
        "30 DEF FNa = 1.5",
        "40 DEF FNb$ = \"x\"",
        "50 DEF FNc% = 7",
        "60 DEF FNt = \"y\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "1.5x7y\n");
}

#[test]
fn test_print_tab_then_string_function() {
    let (result, output) = run(&[
        "10 PRINT TAB(5); FNt",
        "20 END",
        "30 DEF FNt = \"one line\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "     one line\n");
}

#[test]
fn test_print_function_with_argument_and_string_result() {
    let (result, output) = run(&[
        "10 PRINT FNw(3)",
        "20 END",
        "30 DEF FNw(N) = LEFT$(\"***\", N)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "***\n");
}

#[test]
fn test_print_function_that_gives_a_string_function_result() {
    let (result, output) = run(&[
        "10 PRINT FNo",
        "20 END",
        "30 DEF FNo = FNi",
        "40 DEF FNi = \"inner\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "inner\n");
}

#[test]
fn test_print_function_number_formats_do_not_change() {
    let (result, output) = run(&[
        "10 PRINT FNa; \" \"; FNi; \" \"; FNd",
        "20 END",
        "30 DEF FNa = 1/3",
        "40 DEF FNi = 42",
        "50 DEF FNd = 2^31",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0.333333333 42 2.14748365E9\n");
}

#[test]
fn test_print_hex_of_function_does_not_change() {
    let (result, output) = run(&["10 PRINT ~FNa", "20 END", "30 DEF FNa = 255"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "FF\n");
}

#[test]
fn test_assign_string_function_result_still_works() {
    let (result, output) = run(&[
        "10 A$ = FNt : PRINT A$",
        "20 END",
        "30 DEF FNt = \"one line\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "one line\n");
}

#[test]
fn test_print_string_function_plus_a_string() {
    let (result, output) = run(&[
        "10 PRINT FNt + \"x\"; \"x\" + FNt",
        "20 END",
        "30 DEF FNt = \"one line\"",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "one linexxone line\n");
}

#[test]
fn test_print_len_of_string_function() {
    let (result, output) = run(&["10 PRINT LEN(FNt)", "20 END", "30 DEF FNt = \"one line\""]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "8\n");
}
