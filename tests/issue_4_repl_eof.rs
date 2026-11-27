//! Tests that the interpreter binary stops at the end of its input.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn test_binary_stops_at_end_of_input_without_exit() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_bbc-basic-interpreter"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("binary starts");

    // Read stdout on a thread, so that a full pipe cannot block the child.
    let mut stdout = child.stdout.take().expect("stdout is piped");
    let reader = thread::spawn(move || {
        let mut output = String::new();
        let _ = stdout.read_to_string(&mut output);
        output
    });

    {
        let mut stdin = child.stdin.take().expect("stdin is piped");
        stdin
            .write_all(b"10 PRINT \"HELLO\"\nRUN\n")
            .expect("input is written");
        // Dropping stdin closes it, so the interpreter sees end of input.
    }

    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait().expect("child status is readable") {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("interpreter did not stop within 5 seconds of end of input");
        }
        thread::sleep(Duration::from_millis(20));
    };

    let output = reader.join().expect("reader thread completes");
    assert!(status.success(), "exit status was {status}");
    assert!(output.contains("HELLO"), "output was {output:?}");
}
