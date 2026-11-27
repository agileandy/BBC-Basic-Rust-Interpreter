//! Issue #133: OPENIN, OPENOUT and OPENUP work with no bracket.

use crate::common::run_lines as run;
use std::path::PathBuf;

/// Make an empty temporary directory for one test and return a file path in it.
fn temp_file(name: &str) -> String {
    let dir: PathBuf =
        std::env::temp_dir().join(format!("bbc_issue_133_{}_{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir is created");
    dir.join("data.txt").to_string_lossy().into_owned()
}

#[test]
fn test_openout_and_openin_with_string_literal() {
    let file = temp_file("literal");
    let open_out = format!("10 F% = OPENOUT \"{}\"", file);
    let open_in = format!("40 G% = OPENIN \"{}\"", file);
    let (result, output) = run(&[
        &open_out,
        "20 PRINT#F%, \"ABC\"",
        "30 CLOSE#F%",
        &open_in,
        "50 INPUT#G%, A$",
        "60 CLOSE#G%",
        "70 PRINT A$",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "ABC\n");
}

#[test]
fn test_openup_with_string_literal() {
    let file = temp_file("openup_literal");
    let open_out = format!("10 F% = OPENOUT \"{}\"", file);
    let open_up = format!("40 G% = OPENUP \"{}\"", file);
    let (result, output) = run(&[
        &open_out,
        "20 PRINT#F%, \"ABC\"",
        "30 CLOSE#F%",
        &open_up,
        "50 PRINT EXT#G%",
        "60 CLOSE#G%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "4\n");
}

#[test]
fn test_open_functions_with_string_variable() {
    let file = temp_file("variable");
    let name = format!("10 N$ = \"{}\"", file);
    let (result, output) = run(&[
        &name,
        "20 F% = OPENOUT N$",
        "30 BPUT#F%, 65",
        "40 CLOSE#F%",
        "50 G% = OPENIN N$",
        "60 PRINT BGET#G%",
        "70 CLOSE#G%",
        "80 H% = OPENUP N$",
        "90 PRINT EXT#H%",
        "100 CLOSE#H%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "65\n1\n");
}

#[test]
fn test_open_functions_with_string_expression_in_brackets() {
    let file = temp_file("brackets");
    let (dir, leaf) = file.rsplit_once('/').expect("path has a directory");
    let dir_line = format!("10 D$ = \"{}/\"", dir);
    let open_out = format!("20 F% = OPENOUT (D$ + \"{}\")", leaf);
    let open_in = format!("50 G% = OPENIN (D$ + \"{}\")", leaf);
    let open_up = format!("80 H% = OPENUP (D$ + \"{}\")", leaf);
    let (result, output) = run(&[
        &dir_line,
        &open_out,
        "30 BPUT#F%, 66",
        "40 CLOSE#F%",
        &open_in,
        "60 PRINT BGET#G%",
        "70 CLOSE#G%",
        &open_up,
        "90 PRINT EXT#H%",
        "100 CLOSE#H%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "66\n1\n");
}

#[test]
fn test_openin_with_no_bracket_gives_0_for_missing_file() {
    let file = temp_file("missing");
    let open_in = format!("10 A% = OPENIN \"{}.none\"", file);
    let (result, output) = run(&[&open_in, "20 PRINT A%"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "0\n");
}

#[test]
fn test_open_function_takes_only_the_next_factor() {
    // OPENOUT "a" + "b" is (OPENOUT "a") + "b": a number plus a string
    let file = temp_file("next_factor");
    let open_out = format!("10 F% = OPENOUT \"{}\" + \"b\"", file);
    let (result, _) = run(&[&open_out]);

    assert_eq!(result, Err("Type mismatch at line 10".to_string()));
}

#[test]
fn test_variables_that_start_with_the_names_keep_behaviour() {
    let (result, output) = run(&[
        "10 OPENINX = 7",
        "20 DIM OPENOUTA%(2)",
        "30 OPENOUTA%(1) = 3",
        "40 OPENUPS$ = \"Z\"",
        "50 PRINT OPENINX;OPENOUTA%(1);OPENUPS$",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "73Z\n");
}

#[test]
fn test_channel_functions_with_no_bracket() {
    let file = temp_file("channel");
    let open_out = format!("10 F% = OPENOUT \"{}\"", file);
    let open_in = format!("40 G% = OPENIN \"{}\"", file);
    let (result, output) = run(&[
        &open_out,
        "20 BPUT#F%, 65",
        "30 CLOSE#F%",
        &open_in,
        "50 B% = BGET#G%",
        "60 PRINT B%;\" \";PTR#G%;\" \";EXT#G%;\" \";EOF#G%",
        "70 CLOSE#G%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "65 1 1 -1\n");
}
