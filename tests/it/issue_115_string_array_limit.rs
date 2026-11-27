//! Issue #115: an element of a string array has the same length limit as a
//! string variable.

use crate::common::run_with_limit as run;

#[test]
fn test_element_longer_than_the_limit_gives_string_too_long() {
    let (result, output) = run(&[
        "10 DIM A$(2)",
        "20 A$(1) = STRING$(60000, \"A\")",
        "30 A$(1) = A$(1) + A$(1)",
        "40 PRINT LEN(A$(1))",
    ]);
    assert_eq!(result, Err("String too long at line 30".to_string()));
    assert_eq!(output, "");
}

#[test]
fn test_element_of_65535_characters_works() {
    let (result, output) = run(&[
        "10 DIM A$(2)",
        "20 A$(1) = STRING$(65534, \"A\") + \"B\"",
        "30 PRINT LEN(A$(1))",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "     65535\n");
}

#[test]
fn test_element_of_65536_characters_gives_string_too_long() {
    let (result, _) = run(&[
        "10 DIM A$(2)",
        "20 A$(1) = STRING$(65535, \"A\")",
        "30 A$(1) = A$(1) + \"B\"",
    ]);
    assert_eq!(result, Err("String too long at line 30".to_string()));
}

#[test]
fn test_element_keeps_its_old_value_after_the_error() {
    let (result, output) = run(&[
        "10 ON ERROR GOTO 100",
        "20 DIM A$(2)",
        "30 A$(1) = STRING$(60000, \"A\")",
        "40 A$(1) = A$(1) + A$(1)",
        "50 END",
        "100 PRINT REPORT$",
        "110 PRINT LEN(A$(1))",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "String too long\n     60000\n");
}
