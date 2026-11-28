//! Issue 210: play the sound on the default audio output of the computer.
//! No test opens a device: the tests give a fake opener to the factory.

use bbc_basic_interpreter::sound::{
    factory_with, lock, OutputFactory, SharedEngine, SoundEngine, SoundOutput,
};
use bbc_basic_interpreter::ui::{parse_options, Options, Start, USAGE};
use std::io::{self, Read, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|a| a.to_string()).collect()
}

/// A writer for the note, which the test reads after the factory ran.
#[derive(Clone, Default)]
struct Note(Arc<Mutex<Vec<u8>>>);

impl Note {
    fn text(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }
}

impl Write for Note {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A fake device: it takes samples only when the test asks.
struct FakeDevice;

impl SoundOutput for FakeDevice {}

/// Make the output of the factory, let it run 50 ms, stop it, and give the
/// count of samples that it took from the engine.
fn clock_after_a_moment(factory: OutputFactory) -> u64 {
    let engine: SharedEngine = Arc::new(Mutex::new(SoundEngine::default()));
    let mut output = factory(Arc::clone(&engine));
    thread::sleep(Duration::from_millis(50));
    output.stop();
    let clock = lock(&engine).clock();
    clock
}

#[test]
fn the_factory_gives_the_device_that_opens_and_writes_no_note() {
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    fn open(_: SharedEngine) -> Result<Box<dyn SoundOutput>, String> {
        CALLS.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(FakeDevice))
    }
    let note = Note::default();
    let clock = clock_after_a_moment(factory_with(false, open, Box::new(note.clone())));
    assert_eq!(CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(note.text(), "");
    // The fake device takes no samples, so no clock output runs as well
    assert_eq!(clock, 0);
}

#[test]
fn with_no_device_the_sound_is_silent_and_the_note_comes_one_time() {
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    fn open(_: SharedEngine) -> Result<Box<dyn SoundOutput>, String> {
        CALLS.fetch_add(1, Ordering::SeqCst);
        Err("no audio output device".to_string())
    }
    let note = Note::default();
    let clock = clock_after_a_moment(factory_with(false, open, Box::new(note.clone())));
    assert_eq!(CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(note.text(), "No sound: no audio output device\n");
    // The clock output keeps the time of the queues: 50 ms is 2205 samples
    assert!(clock > 0, "took {clock} samples");
}

#[test]
fn no_sound_makes_no_device_call_and_writes_no_note() {
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    fn open(_: SharedEngine) -> Result<Box<dyn SoundOutput>, String> {
        CALLS.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(FakeDevice))
    }
    let note = Note::default();
    let clock = clock_after_a_moment(factory_with(true, open, Box::new(note.clone())));
    assert_eq!(CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(note.text(), "");
    assert!(clock > 0, "took {clock} samples");
}

#[test]
fn no_sound_flag_turns_the_sound_off_at_the_prompt_and_in_the_ide() {
    let cases = [
        (&["--no-sound"][..], Start::Prompt),
        (&["--no-sound", "--ide"][..], Start::Ide(None)),
        (&["--ide", "--no-sound"][..], Start::Ide(None)),
        (
            &["--no-sound", "--ide", "a.bbas"][..],
            Start::Ide(Some("a.bbas".to_string())),
        ),
        (
            &["--ide", "a.bbas", "--no-sound"][..],
            Start::Ide(Some("a.bbas".to_string())),
        ),
    ];
    for (list, start) in cases {
        assert_eq!(
            parse_options(args(list)),
            Ok(Options {
                start,
                no_sound: true
            }),
            "{list:?}"
        );
    }
}

#[test]
fn with_no_flag_the_sound_is_on() {
    assert_eq!(
        parse_options(args(&["--ide"])),
        Ok(Options {
            start: Start::Ide(None),
            no_sound: false
        })
    );
}

#[test]
fn the_usage_line_shows_the_no_sound_flag() {
    assert_eq!(
        USAGE,
        "Usage: bbc-basic-interpreter [--no-sound] [--ide [file]]"
    );
    for list in [
        &["--no-sound", "--no-sound"][..],
        &["--no-sound", "--foo"][..],
    ] {
        assert_eq!(parse_options(args(list)), Err(USAGE), "{list:?}");
    }
}

/// Run the binary with `args`, write `input`, and give the exit code, the
/// standard output and the standard error. The readers run while the input
/// goes in, so a full pipe cannot block the binary.
fn run_binary(args: &[&str], input: &str) -> (Option<i32>, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_bbc-basic-interpreter"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("binary starts");
    let mut stdout = child.stdout.take().expect("stdout is piped");
    let mut stderr = child.stderr.take().expect("stderr is piped");
    let out = thread::spawn(move || {
        let mut text = String::new();
        let _ = stdout.read_to_string(&mut text);
        text
    });
    let err = thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });
    let _ = child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(input.as_bytes());
    let status = child.wait().expect("binary ends");
    (
        status.code(),
        out.join().expect("stdout reader"),
        err.join().expect("stderr reader"),
    )
}

#[test]
fn the_prompt_runs_with_the_no_sound_flag() {
    let (code, stdout, stderr) = run_binary(&["--no-sound"], "PRINT 6*7\nEXIT\n");
    assert_eq!(code, Some(0));
    assert!(stdout.contains("42\n"), "{stdout}");
    assert!(stdout.contains("Goodbye!"), "{stdout}");
    assert_eq!(stderr, "");
}
