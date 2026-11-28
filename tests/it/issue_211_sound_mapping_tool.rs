//! Issue #211: the sound mapping tool (`--sound-map`), the recording of the
//! sound of a run to a WAV file (`--sound-wav`), and `NOTE(name$)`.

use crate::common::run_lines;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Run the binary with `args`, read stdout on a thread, then write `input`.
/// Kill the binary after `limit`. Give the exit code and stdout.
fn run_binary_with_args(args: &[&str], input: &str, limit: Duration) -> (Option<i32>, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_bbc-basic-interpreter"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("binary starts");
    let mut stdout = child.stdout.take().expect("stdout is piped");
    let reader = thread::spawn(move || {
        let mut output = String::new();
        let _ = stdout.read_to_string(&mut output);
        output
    });
    let _ = child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(input.as_bytes());

    let deadline = Instant::now() + limit;
    let status = loop {
        if let Some(status) = child.try_wait().expect("child status is readable") {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("interpreter did not stop within {limit:?}");
        }
        thread::sleep(Duration::from_millis(20));
    };
    (
        status.code(),
        reader.join().expect("reader thread completes"),
    )
}

/// A file name in the temporary directory that only this test uses.
fn temp_file(test: &str) -> PathBuf {
    std::env::temp_dir().join(format!("bbc_basic_211_{}_{}.wav", test, std::process::id()))
}

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

#[test]
fn sound_wav_records_one_note_of_one_second() {
    let path = temp_file("one_note");
    let path_text = path.to_str().expect("a text path");

    let (code, output) = run_binary_with_args(
        &["--no-sound", "--sound-wav", path_text],
        "10 SOUND 1,-15,89,20\nRUN\nEXIT\n",
        Duration::from_secs(10),
    );
    let bytes = std::fs::read(&path).expect("the WAV file exists");
    let _ = std::fs::remove_file(&path);

    assert_eq!(code, Some(0), "output: {output}");
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..16], b"WAVEfmt ");
    assert_eq!(u16_at(&bytes, 20), 1, "PCM");
    assert_eq!(u16_at(&bytes, 22), 1, "mono");
    assert_eq!(u16_at(&bytes, 34), 16, "16 bits");
    assert_eq!(&bytes[36..40], b"data");
    let rate = u32_at(&bytes, 24);
    let data = u32_at(&bytes, 40);
    assert_eq!(u32_at(&bytes, 4), data + 36, "RIFF size");
    assert_eq!(bytes.len() as u32, data + 44, "the data size is the file");

    // The note plays for 1 second (duration 20). The interpreter waits for
    // the sound to end before it ends the file.
    let samples = data / 2;
    let error = (f64::from(samples) - f64::from(rate)).abs() / f64::from(rate);
    assert!(
        error <= 0.02,
        "{samples} samples at {rate} each second: {:.1} percent",
        error * 100.0
    );
    let loud = bytes[44..]
        .chunks(2)
        .filter(|pair| i16::from_le_bytes([pair[0], pair[1]]) != 0)
        .count();
    assert!(loud > samples as usize / 2, "the note is in the file");
}

#[test]
fn sound_wav_gives_an_empty_valid_file_for_a_run_with_no_sound() {
    let path = temp_file("no_sound");
    let path_text = path.to_str().expect("a text path");

    let (code, output) = run_binary_with_args(
        &["--no-sound", "--sound-wav", path_text],
        "PRINT 1+1\nEXIT\n",
        Duration::from_secs(5),
    );
    let bytes = std::fs::read(&path).expect("the WAV file exists");
    let _ = std::fs::remove_file(&path);

    assert_eq!(code, Some(0));
    assert!(output.contains('2'), "output: {output}");
    assert_eq!(bytes.len(), 44);
    assert_eq!(u32_at(&bytes, 40), 0);
}

#[test]
fn sound_wav_with_a_path_that_cannot_be_created_is_an_error() {
    let (code, _) = run_binary_with_args(
        &["--sound-wav", "/no/such/directory/x.wav"],
        "EXIT\n",
        Duration::from_secs(5),
    );
    assert_eq!(code, Some(2));
}

#[test]
fn sound_map_prints_a_table_of_256_pitch_values() {
    let (code, output) = run_binary_with_args(&["--sound-map"], "", Duration::from_secs(5));

    assert_eq!(code, Some(0));
    let rows: Vec<&str> = output
        .lines()
        .filter(|line| line.contains(" Hz "))
        .collect();
    assert_eq!(rows.len(), 256, "{output}");
    // Pitch 53 is the User Guide C4. Its real pitch is C4 +32 cents.
    let row_53: Vec<&str> = rows[53].split_whitespace().collect();
    assert_eq!(
        row_53,
        vec!["53", "469", "266.52", "Hz", "C4", "+32.1", "C4"]
    );
    assert!(output.contains("quarter of a semitone sharp"));
}

#[test]
fn sound_map_c4_gives_52_first() {
    let (code, output) = run_binary_with_args(&["--sound-map", "C4"], "", Duration::from_secs(5));

    assert_eq!(code, Some(0));
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(
        lines[0],
        "C4 is 261.63 Hz. The nearest pitch values on channel 1:"
    );
    let first: Vec<&str> = lines[2].split_whitespace().collect();
    assert_eq!(first, vec!["52", "263.16", "Hz", "+10.1", "C4", "B3+3/4"]);
}

#[test]
fn sound_map_with_a_wrong_note_is_an_error() {
    let (code, output) = run_binary_with_args(&["--sound-map", "H4"], "", Duration::from_secs(5));
    assert_eq!(code, Some(2));
    assert_eq!(output, "");
}

#[test]
fn note_gives_the_pitch_value_of_a_note_name() {
    let (result, output) = run_lines(&[
        "10 PRINT STR$(NOTE(\"C4\")); \" \"; STR$(NOTE(\"A4\")); \" \"; STR$(NOTE(\"F#3\"))",
        "20 N$ = \"c4\" : SOUND &2001, -15, NOTE(N$), 1",
        "30 PRINT STR$(NOTE(\"Bb3\") - NOTE(\"A#3\"))",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "52 88 28\n0\n");
}

#[test]
fn note_with_a_wrong_name_is_an_error_that_on_error_traps() {
    let (result, _) = run_lines(&["10 PRINT NOTE(\"H4\")"]);
    assert_eq!(result, Err("Bad note: H4 at line 10".to_string()));

    let (result, output) = run_lines(&[
        "10 ON ERROR PRINT REPORT$; \" \"; STR$(ERR) : END",
        "20 X = NOTE(\"C1\")",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "Bad note: C1 159\n");
}

#[test]
fn a_variable_and_an_array_named_note_still_work() {
    let (result, output) = run_lines(&[
        "10 NOTE = 7 : PRINT STR$(NOTE)",
        "20 DIM NOTE(3) : NOTE(2) = 5 : PRINT STR$(NOTE(2) + NOTE)",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "7\n12\n");
}

#[test]
fn the_wait_for_the_sound_ends_when_the_output_takes_no_samples() {
    use bbc_basic_interpreter::commands::run_command;
    use bbc_basic_interpreter::executor::Executor;
    use bbc_basic_interpreter::program::ProgramStore;
    use bbc_basic_interpreter::sound::SoundOutput;

    // An output that never takes a sample, as a device that does not run
    struct Stalled;
    impl SoundOutput for Stalled {}

    let mut executor = Executor::new();
    executor.set_sound_output(Box::new(|_| Box::new(Stalled) as Box<dyn SoundOutput>));
    let mut program = ProgramStore::new();
    run_command(&mut executor, &mut program, "SOUND 1,-15,88,20").expect("SOUND runs");

    let start = Instant::now();
    executor.wait_for_sound(Duration::from_secs(20));
    assert!(
        start.elapsed() < Duration::from_secs(3),
        "waited {:?}",
        start.elapsed()
    );
}
