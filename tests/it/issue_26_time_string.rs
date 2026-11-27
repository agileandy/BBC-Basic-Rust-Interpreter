//! Issue 26: TIME$ gives the date and time as text.

use crate::common::run_lines as run;

#[test]
fn test_time_string_has_24_characters() {
    let (result, output) = run(&["10 PRINT LEN(TIME$)"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "24");
}

#[test]
fn test_time_string_has_a_comma_after_the_day() {
    let (result, output) = run(&["10 A$ = TIME$ : PRINT MID$(A$, 4, 1)"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, ",\n");
}

#[test]
fn test_time_string_has_the_master_format() {
    let (result, output) = run(&["10 PRINT TIME$"]);

    assert_eq!(result, Ok(()));
    let text: Vec<char> = output.trim_end().chars().collect();
    assert_eq!(text.len(), 24, "output was {:?}", output);
    let days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let day: String = text[0..3].iter().collect();
    assert!(days.contains(&day.as_str()), "output was {:?}", output);
    for (index, expected) in [
        (3, ','),
        (6, ' '),
        (10, ' '),
        (15, '.'),
        (18, ':'),
        (21, ':'),
    ] {
        assert_eq!(text[index], expected, "output was {:?}", output);
    }
}

#[test]
fn test_time_still_gives_a_number() {
    let (result, output) = run(&["10 T% = TIME : PRINT T% >= 0"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output.trim(), "-1");
}

#[test]
fn test_assignment_to_time_string_gives_an_error() {
    let (result, _output) = run(&["10 TIME$ = \"Fri,27 Nov 2026.14:05:09\""]);

    let message = result.expect_err("assignment to TIME$ must fail");
    assert!(message.contains("TIME$"), "message was {:?}", message);
}
