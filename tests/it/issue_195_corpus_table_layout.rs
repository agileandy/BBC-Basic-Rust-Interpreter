//! Issue #195: the corpus programs print their tables as their authors
//! intended, with the PRINT field of 10 characters from issue #91.
//!
//! A program that wants no padding uses `PRINT ;N%`. A program that uses the
//! field on purpose prints only fields on that line, for example
//! `PRINT A, B`. So a line of an expected file that starts with a number in a
//! field of 10 must contain only fields of 10.

use std::path::{Path, PathBuf};

/// The count of corpus programs. Issue #195 changes programs but does not
/// add or remove one.
const PROGRAM_COUNT: usize = 31;

/// The program that issue #166 owns. It sets `@%`, so its fields are not 10.
const NOT_CHECKED: &str = "at_percent_format";

/// The width of a PRINT field.
const FIELD: usize = 10;

/// Give the corpus directory.
fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus")
}

/// Give the path of each file in the corpus with the extension.
fn corpus_files(extension: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(corpus_dir())
        .expect("corpus directory exists")
        .map(|entry| entry.expect("corpus entry reads").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == extension))
        .collect();
    files.sort();
    files
}

/// Give true if the text is a number at the right of one field of 10.
fn is_number_field(text: &str) -> bool {
    let number = text.trim_start();
    text.len() == FIELD
        && text.starts_with(' ')
        && !number.contains(' ')
        && number.starts_with(|c: char| c.is_ascii_digit() || c == '-')
}

/// Give true if the line starts with a number in a field of 10 and has text
/// after it that is not more fields of 10.
fn is_broken_field_line(line: &str) -> bool {
    if !line.is_ascii() || line.len() < FIELD || !is_number_field(&line[..FIELD]) {
        return false;
    }
    !line.len().is_multiple_of(FIELD)
        || !line
            .as_bytes()
            .chunks(FIELD)
            .all(|chunk| is_number_field(std::str::from_utf8(chunk).expect("ASCII line")))
}

#[test]
fn test_the_corpus_has_the_same_count_of_programs() {
    let programs = corpus_files("bas");
    assert_eq!(programs.len(), PROGRAM_COUNT, "{:?}", programs);
    for program in programs {
        assert!(
            program.with_extension("out").is_file(),
            "{} has an expected file",
            program.display()
        );
    }
}

#[test]
fn test_a_line_that_starts_with_a_number_field_has_only_fields() {
    let mut broken = Vec::new();
    for path in corpus_files("out") {
        let name = path.file_stem().expect("file name").to_string_lossy();
        if name == NOT_CHECKED {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("expected file reads");
        for (index, line) in text.lines().enumerate() {
            if is_broken_field_line(line) {
                broken.push(format!("{}:{}: {:?}", name, index + 1, line));
            }
        }
    }
    assert!(
        broken.is_empty(),
        "lines with a broken layout:\n{}",
        broken.join("\n")
    );
}

#[test]
fn test_the_check_finds_a_field_followed_by_text() {
    assert!(is_broken_field_line("         7 + 5 = 12"));
    assert!(is_broken_field_line("         1          4"));
    assert!(!is_broken_field_line("         1        24"));
    assert!(!is_broken_field_line("       1.5"));
    assert!(!is_broken_field_line("         N        N!"));
    assert!(!is_broken_field_line("A42               42"));
    assert!(!is_broken_field_line("7 + 5 = 12"));
}
