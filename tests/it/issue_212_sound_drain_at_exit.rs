//! Issue 212: at EXIT, QUIT and the end of standard input, the interpreter
//! waits for the notes in the sound queues to end. An audio device that
//! takes no samples changes to the clock output, so the notes still end.

use bbc_basic_interpreter::sound::{factory_with, lock, SharedEngine, SoundEngine, SoundOutput};
use std::io::{self, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// The longest run of the binary in a test.
const RUN_LIMIT: Duration = Duration::from_secs(10);

/// Three notes of 5 twentieths of a second on channel 1: 0.75 s of sound.
const THREE_NOTES: &str =
    "10 FOR P%=53 TO 61 STEP 4\n20 SOUND 1,-15,P%,5\n30 NEXT\n40 PRINT \"END\"\nRUN\n";

/// Start the binary with `args` and `--no-sound`, write the input, and
/// close the input.
fn start(args: &[&str], input: &str) -> Child {
    let mut child = Command::new(env!("CARGO_BIN_EXE_bbc-basic-interpreter"))
        .arg("--no-sound")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("binary starts");
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(input.as_bytes())
        .expect("input is written");
    child
}

/// Wait for the binary to end, and give its output and the time from
/// `started`. Kill it after `RUN_LIMIT`.
fn finish(mut child: Child, started: Instant) -> (String, Duration) {
    let mut stdout = child.stdout.take().expect("stdout is piped");
    let reader = thread::spawn(move || {
        let mut output = String::new();
        let _ = stdout.read_to_string(&mut output);
        output
    });
    while child.try_wait().expect("child status").is_none() {
        if started.elapsed() >= RUN_LIMIT {
            let _ = child.kill();
            let _ = child.wait();
            panic!("interpreter did not stop within {RUN_LIMIT:?}");
        }
        thread::sleep(Duration::from_millis(10));
    }
    let elapsed = started.elapsed();
    (reader.join().expect("reader ends"), elapsed)
}

/// Run the binary to its end, and give its output and its run time.
fn run_timed(args: &[&str], input: &str) -> (String, Duration) {
    let started = Instant::now();
    let child = start(args, input);
    finish(child, started)
}

#[test]
fn exit_waits_for_the_queued_notes_to_end() {
    let (output, elapsed) = run_timed(&[], &format!("{THREE_NOTES}EXIT\n"));
    assert!(output.contains("END\n"), "output: {output}");
    assert!(output.contains("Goodbye!"), "output: {output}");
    assert!(
        elapsed >= Duration::from_millis(700),
        "ended after {elapsed:?}"
    );
}

#[test]
fn quit_waits_for_the_queued_notes_to_end() {
    let (output, elapsed) = run_timed(&[], &format!("{THREE_NOTES}QUIT\n"));
    assert!(output.contains("END\n"), "output: {output}");
    assert!(
        elapsed >= Duration::from_millis(700),
        "ended after {elapsed:?}"
    );
}

#[test]
fn the_end_of_input_waits_for_the_queued_notes_to_end() {
    let (output, elapsed) = run_timed(&[], THREE_NOTES);
    assert!(output.contains("END\n"), "output: {output}");
    assert!(
        elapsed >= Duration::from_millis(700),
        "ended after {elapsed:?}"
    );
}

#[test]
fn a_note_of_duration_255_does_not_make_the_wait() {
    let (output, elapsed) = run_timed(&[], "SOUND 1,-15,53,255\nEXIT\n");
    assert!(output.contains("Goodbye!"), "output: {output}");
    assert!(elapsed < Duration::from_secs(3), "ended after {elapsed:?}");
}

#[test]
fn an_interrupt_during_the_wait_ends_the_interpreter_at_once() {
    // 5 seconds of sound
    let started = Instant::now();
    let child = start(&[], "SOUND 1,-15,53,100\nEXIT\n");
    thread::sleep(Duration::from_millis(500));
    let sent = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .expect("kill runs");
    assert!(sent.success());
    let (_, elapsed) = finish(child, started);
    assert!(elapsed < Duration::from_secs(2), "ended after {elapsed:?}");
}

#[test]
fn with_no_sound_the_recording_holds_the_full_tune() {
    let wav = std::env::temp_dir().join(format!("issue_212_full_tune_{}.wav", std::process::id()));
    let wav_name = wav.to_str().expect("path is text");
    let (output, _) = run_timed(&["--sound-wav", wav_name], &format!("{THREE_NOTES}EXIT\n"));
    let bytes = std::fs::read(&wav).expect("the WAV file exists");
    let _ = std::fs::remove_file(&wav);
    assert!(output.contains("END\n"), "output: {output}");

    // A WAV file of 16-bit mono samples: the byte count of the data chunk
    // at bytes 40 to 43, and the sample rate at bytes 24 to 27
    let data_bytes = u32::from_le_bytes(bytes[40..44].try_into().unwrap());
    let rate = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
    let seconds = f64::from(data_bytes) / 2.0 / f64::from(rate);
    assert!(seconds >= 0.7, "the recording has {seconds} s");
}

/// A writer for the note, which the test reads after the output ran.
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

/// A fake device that opens but takes no samples.
struct SilentDevice;

impl SoundOutput for SilentDevice {}

fn open_silent(_: SharedEngine) -> Result<Box<dyn SoundOutput>, String> {
    Ok(Box::new(SilentDevice))
}

/// An engine with a note of one second on channel 1.
fn engine_with_a_note() -> SharedEngine {
    let engine: SharedEngine = Arc::new(Mutex::new(SoundEngine::default()));
    assert!(lock(&engine).try_sound(1, -15, 89, 20));
    engine
}

#[test]
fn a_device_that_takes_no_samples_changes_to_the_clock_output() {
    let note = Note::default();
    let engine = engine_with_a_note();
    let mut output = factory_with(false, open_silent, Box::new(note.clone()))(Arc::clone(&engine));

    thread::sleep(Duration::from_millis(1200));
    let clock = lock(&engine).clock();
    thread::sleep(Duration::from_millis(600));
    output.stop();

    assert!(clock > 0, "the clock output takes samples");
    assert_eq!(
        note.text(),
        "No sound: the audio device takes no samples\n",
        "the note comes one time"
    );
}

#[test]
fn a_device_with_no_note_to_play_does_not_change() {
    let note = Note::default();
    let engine: SharedEngine = Arc::new(Mutex::new(SoundEngine::default()));
    let mut output = factory_with(false, open_silent, Box::new(note.clone()))(Arc::clone(&engine));

    thread::sleep(Duration::from_millis(800));
    output.stop();

    assert_eq!(lock(&engine).clock(), 0);
    assert_eq!(note.text(), "");
}

/// A fake device that reports a stream error at once.
struct FailedDevice;

impl SoundOutput for FailedDevice {
    fn has_failed(&self) -> bool {
        true
    }
}

#[test]
fn a_stream_error_changes_to_the_clock_output() {
    fn open(_: SharedEngine) -> Result<Box<dyn SoundOutput>, String> {
        Ok(Box::new(FailedDevice))
    }
    let note = Note::default();
    let engine = engine_with_a_note();
    let mut output = factory_with(false, open, Box::new(note.clone()))(Arc::clone(&engine));

    thread::sleep(Duration::from_millis(300));
    let clock = lock(&engine).clock();
    output.stop();

    assert!(clock > 0, "the clock output takes samples before 500 ms");
    assert_eq!(note.text(), "No sound: the audio device stopped\n");
}

/// A fake device that takes samples on its own thread, as a device does.
struct WorkingDevice {
    stop: Arc<AtomicBool>,
}

impl SoundOutput for WorkingDevice {
    fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

#[test]
fn a_device_that_takes_samples_does_not_change() {
    fn open(engine: SharedEngine) -> Result<Box<dyn SoundOutput>, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        thread::spawn(move || {
            let mut samples = vec![0.0f32; 441];
            while !flag.load(Ordering::SeqCst) {
                lock(&engine).fill(&mut samples);
                thread::sleep(Duration::from_millis(10));
            }
        });
        Ok(Box::new(WorkingDevice { stop }))
    }
    let note = Note::default();
    let engine = engine_with_a_note();
    let mut output = factory_with(false, open, Box::new(note.clone()))(Arc::clone(&engine));

    thread::sleep(Duration::from_millis(800));
    output.stop();

    assert_eq!(note.text(), "");
}
