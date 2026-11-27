//! Tests that the interpreter binary ends with no panic text when its output closes early.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn test_binary_ends_without_panic_when_output_closes() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_bbc-basic-interpreter"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("binary starts");

    // Read stderr on a thread, so that a full pipe cannot block the child.
    let mut stderr = child.stderr.take().expect("stderr is piped");
    let error_reader = thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });

    {
        // 5000 lines of 50 characters are more than a pipe buffer holds,
        // so the interpreter writes after the read end closes.
        let mut stdin = child.stdin.take().expect("stdin is piped");
        stdin
            .write_all(
                b"10 FOR I% = 1 TO 5000\n\
                  20 PRINT \"ABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVW\"\n\
                  30 NEXT I%\n\
                  RUN\n\
                  EXIT\n",
            )
            .expect("input is written");
    }

    {
        let stdout = child.stdout.take().expect("stdout is piped");
        let mut first_line = String::new();
        BufReader::new(stdout)
            .read_line(&mut first_line)
            .expect("first line is readable");
        assert!(!first_line.is_empty(), "no output line was read");
        // Dropping the reader closes the read end of the pipe.
    }

    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait().expect("child status is readable") {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("interpreter did not stop within 5 seconds of the output closing");
        }
        thread::sleep(Duration::from_millis(20));
    };

    let errors = error_reader.join().expect("stderr reader completes");
    assert!(!errors.contains("panicked"), "stderr was {errors:?}");
    assert!(status.success(), "exit status was {status}");
}
