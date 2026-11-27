//! Issue #156: a corpus of short programs in the normal BBC BASIC style.
//!
//! Each program is `tests/corpus/<name>.bas`, and its expected output is
//! `tests/corpus/<name>.out`. The expected output of each program was written
//! by hand from the BBC BASIC rules. Each program runs with a time limit.
//!
//! A program that uses a file has the text `@TEMP@` in its file name. The test
//! replaces that text with a directory that is unique for the program.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::common::run_with_replies;

/// The text in a program that the test replaces with a temporary directory.
const TEMP_MARK: &str = "@TEMP@";

/// Run the program text with the replies on a thread, with a limit of 5
/// seconds. Give the result and the output.
fn run_corpus(name: &str, source: &str, replies: &[&str]) -> (Result<(), String>, String) {
    let lines: Vec<String> = source
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_string)
        .collect();
    let replies: Vec<String> = replies.iter().map(|reply| reply.to_string()).collect();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        let replies: Vec<&str> = replies.iter().map(String::as_str).collect();
        let _ = sender.send(run_with_replies(&lines, &replies));
    });
    receiver
        .recv_timeout(Duration::from_secs(5))
        .unwrap_or_else(|_| panic!("{name} ends within 5 seconds and does not panic"))
}

/// Run one corpus program and compare its output with the expected output.
fn check(name: &str, source: &str, expected: &str, replies: &[&str]) {
    let dir = std::env::temp_dir().join(format!("bbc_issue_156_{}_{}", name, std::process::id()));
    let source = if source.contains(TEMP_MARK) {
        std::fs::create_dir_all(&dir).expect("temporary directory is made");
        source.replace(TEMP_MARK, &dir.to_string_lossy())
    } else {
        source.to_string()
    };

    let (result, output) = run_corpus(name, &source, replies);
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(result, Ok(()), "{name}: result, output so far:\n{output}");
    assert_eq!(output, expected, "{name}: output");
}

/// Make one test for the corpus program `$name`, with optional INPUT replies.
macro_rules! corpus {
    ($(#[$attr:meta])* $name:ident) => {
        corpus!($(#[$attr])* $name, &[]);
    };
    ($(#[$attr:meta])* $name:ident, $replies:expr) => {
        $(#[$attr])*
        #[test]
        fn $name() {
            check(
                stringify!($name),
                include_str!(concat!("../corpus/", stringify!($name), ".bas")),
                include_str!(concat!("../corpus/", stringify!($name), ".out")),
                $replies,
            );
        }
    };
}

corpus!(data_table_1d);
corpus!(data_table_2d);
corpus!(data_read_into_array);
corpus!(bubble_sort);
corpus!(string_sort);
corpus!(menu_on_goto, &["1", "50", "4", "2", "500", "2", "30", "3"]);
corpus!(string_functions);
corpus!(word_split);
corpus!(numeric_functions);
corpus!(recursive_fn);
corpus!(recursive_proc);
corpus!(repeat_flag);
corpus!(file_write_read);
corpus!(file_input_array);
corpus!(graphics_points);
corpus!(tab_columns);
corpus!(
    #[ignore = "defect F1: @% is not a variable, so a program cannot set the number format"]
    at_percent_format
);
corpus!(nested_for_step);
corpus!(gosub_table);
corpus!(if_else_multi);
corpus!(int_div_mod);
corpus!(bit_operations);
corpus!(rem_at_line_end);
corpus!(local_arrays);
corpus!(on_error_trap);
corpus!(restore_lookup);
corpus!(print_separators);
corpus!(bit_masks_not);
corpus!(
    #[ignore = "defect F5: IF with no THEN gives a syntax error"]
    if_without_then
);
corpus!(graphics_count);
corpus!(file_records);
