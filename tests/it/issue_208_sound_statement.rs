//! Issue #208: the SOUND statement, its queues and its clock, and ADVAL(-5)
//! to ADVAL(-8). The rules are those of MOS 1.20 (see `src/sound/engine.rs`).
//!
//! A fake output takes samples only when a test asks, or one tick each time
//! a SOUND waits for a place in a full queue. The timing is thus exact.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use bbc_basic_interpreter::executor::{Console, Executor, Interrupt};
use bbc_basic_interpreter::parser::{parse_expression, parse_statement, Statement};
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::sound::{lock, SharedEngine, SoundOutput};
use bbc_basic_interpreter::tokenizer::tokenize;

use crate::common::{run_binary_read_first, run_with_limit};

/// The samples of one tick (1/100 second) at 44100 samples each second.
const TICK: u64 = 441;

/// An output that takes one tick of samples each time a SOUND waits (if
/// `advance`), and no other samples.
struct FakeOutput {
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl FakeOutput {
    fn start(engine: SharedEngine, advance: bool) -> Self {
        lock(&engine).record_notes();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let thread = thread::spawn(move || {
            let mut buffer = vec![0.0f32; TICK as usize];
            while !flag.load(Ordering::SeqCst) {
                {
                    let mut engine = lock(&engine);
                    if advance && engine.is_waiting() {
                        engine.fill(&mut buffer);
                        engine.set_waiting(false);
                    }
                }
                thread::sleep(Duration::from_millis(1));
            }
        });
        Self {
            stop,
            thread: Some(thread),
        }
    }
}

impl SoundOutput for FakeOutput {
    fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// A console that keeps the text, and gives Escape while a SOUND waits.
struct EscapeWhenWaiting {
    text: Arc<Mutex<String>>,
    engine: Arc<Mutex<Option<SharedEngine>>>,
}

impl Console for EscapeWhenWaiting {
    fn write(&mut self, text: &str) {
        self.text.lock().unwrap().push_str(text);
    }
    fn clear(&mut self) {}
    fn read_line(&mut self) -> Option<String> {
        None
    }
    fn read_key(&mut self, _limit: Option<Duration>) -> Option<char> {
        None
    }
    fn interrupt(&mut self) -> Option<Interrupt> {
        let engine = self.engine.lock().unwrap().clone()?;
        let waiting = lock(&engine).is_waiting();
        waiting.then_some(Interrupt::Escape)
    }
}

/// The result of a run with the fake output.
struct SoundRun {
    result: Result<(), String>,
    text: String,
    engine: SharedEngine,
}

impl SoundRun {
    /// Take `ticks` ticks of samples after the run.
    fn advance(&self, ticks: u64) {
        let mut buffer = vec![0.0f32; (ticks * TICK) as usize];
        lock(&self.engine).fill(&mut buffer);
    }

    /// The (channel, pitch, start tick) of each note that started.
    fn notes(&self) -> Vec<(u8, u8, u64)> {
        lock(&self.engine)
            .take_notes()
            .iter()
            .map(|note| (note.channel, note.pitch, note.start / TICK))
            .collect()
    }
}

/// Run the program with the fake output, on a thread with a time limit of
/// 5 seconds. With `escape`, the console gives Escape while a SOUND waits.
fn run_sound(source: &[&str], escape: bool) -> SoundRun {
    let lines: Vec<String> = source.iter().map(|line| line.to_string()).collect();
    let slot: Arc<Mutex<Option<SharedEngine>>> = Arc::new(Mutex::new(None));
    let text = Arc::new(Mutex::new(String::new()));
    let (sender, receiver) = mpsc::channel();
    let (factory_slot, console_slot, console_text) =
        (Arc::clone(&slot), Arc::clone(&slot), Arc::clone(&text));
    thread::spawn(move || {
        let mut executor = Executor::new();
        executor.set_sound_output(Box::new(move |engine: SharedEngine| {
            *factory_slot.lock().unwrap() = Some(Arc::clone(&engine));
            Box::new(FakeOutput::start(engine, !escape)) as Box<dyn SoundOutput>
        }));
        if escape {
            executor.set_console(Box::new(EscapeWhenWaiting {
                text: console_text,
                engine: console_slot,
            }));
        }
        let mut program = ProgramStore::new();
        for line in &lines {
            program.store_line(tokenize(line).expect("line tokenizes"));
        }
        let result = run_program(&mut executor, &mut program);
        executor.stop_sound();
        let _ = sender.send((result, executor.get_output().to_string()));
    });
    let (result, output) = receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("the program stops in 5 seconds");
    let engine = slot.lock().unwrap().clone().expect("SOUND made an output");
    let console_text = text.lock().unwrap().clone();
    SoundRun {
        result,
        text: output + &console_text,
        engine,
    }
}

/// The expression of a source text. A PRINT in front of it stops the
/// tokenizer from reading a number at the start as a line number.
fn expression(source: &str) -> bbc_basic_interpreter::Expression {
    parse_expression(&tokenize(&format!("PRINT {source}")).unwrap().tokens[1..]).unwrap()
}

#[test]
fn sound_parses_4_values() {
    let statement = parse_statement(&tokenize("SOUND 1, -15, 53, 20").unwrap()).unwrap();

    assert_eq!(
        statement,
        Statement::Sound {
            channel: expression("1"),
            amplitude: expression("-15"),
            pitch: expression("53"),
            duration: expression("20"),
        }
    );
}

#[test]
fn sound_parses_an_expression_in_each_value() {
    let statement =
        parse_statement(&tokenize("SOUND C%+&10, A%-1, P%*4, D DIV 2").unwrap()).unwrap();

    assert_eq!(
        statement,
        Statement::Sound {
            channel: expression("C%+&10"),
            amplitude: expression("A%-1"),
            pitch: expression("P%*4"),
            duration: expression("D DIV 2"),
        }
    );
}

#[test]
fn sound_with_a_wrong_count_of_values_is_a_syntax_error() {
    for (line, count) in [
        ("10 SOUND", 0),
        ("10 SOUND 1, -15, 53", 3),
        ("10 SOUND 1, -15, 53, 20, 5", 5),
    ] {
        let (result, _) = run_with_limit(&[line]);
        assert_eq!(
            result,
            Err(format!(
                "Syntax error at line 10: SOUND requires 4 parameters \
                 (channel, amplitude, pitch, duration), got {count}"
            )),
            "{line}"
        );
    }
}

#[test]
fn three_notes_on_one_channel_play_one_after_the_other() {
    let run = run_sound(
        &[
            "10 SOUND 1, -15, 53, 2",
            "20 SOUND 1, -15, 69, 2",
            "30 SOUND 1, -15, 89, 2",
        ],
        false,
    );
    assert_eq!(run.result, Ok(()));

    run.advance(40);

    // MOS 1.20: a step of a duration is 5 ticks, and the first note ends one
    // tick early because the tick that reads it also counts
    assert_eq!(run.notes(), vec![(1, 53, 0), (1, 69, 9), (1, 89, 19)]);
}

#[test]
fn a_sixth_note_waits_until_a_place_is_free() {
    let run = run_sound(
        &[
            "10 FOR I% = 1 TO 6",
            "20 SOUND 2, -10, 100 + I%, 1",
            "30 NEXT",
            "40 PRINT STR$(ADVAL(-7))",
        ],
        false,
    );
    assert_eq!(run.result, Ok(()));

    // A queue holds 5 notes. The sixth SOUND waited for one tick, which
    // started the first note and freed its place.
    assert_eq!(run.text, "0\n");
    assert_eq!(lock(&run.engine).clock(), TICK);
    assert_eq!(run.notes(), vec![(2, 101, 0)]);
}

#[test]
fn the_flush_bit_empties_the_queue() {
    let run = run_sound(
        &[
            "10 SOUND 3, -15, 100, 20",
            "20 SOUND 3, -15, 110, 20",
            "30 SOUND &13, -15, 120, 20",
            "40 PRINT STR$(ADVAL(-8))",
        ],
        false,
    );
    assert_eq!(run.result, Ok(()));
    assert_eq!(run.text, "12\n");

    run.advance(300);

    assert_eq!(run.notes(), vec![(3, 120, 0)]);
}

#[test]
fn the_sync_bits_start_2_channels_together() {
    let run = run_sound(
        &["10 SOUND &101, -15, 100, 5", "20 SOUND &102, -15, 120, 5"],
        false,
    );
    assert_eq!(run.result, Ok(()));

    run.advance(3);

    // Tick 0 finds both notes of the sync, and tick 1 starts them
    assert_eq!(run.notes(), vec![(2, 120, 1), (1, 100, 1)]);
}

#[test]
fn adval_gives_the_free_places_of_each_queue() {
    let run = run_sound(
        &[
            "10 SOUND 0, -15, 4, 10",
            "20 SOUND 1, -15, 53, 10",
            "30 SOUND 1, -15, 53, 10",
            "40 PRINT STR$(ADVAL(-5)) \" \" STR$(ADVAL(-6)) \" \" STR$(ADVAL(-7)) \" \" STR$(ADVAL(-8))",
        ],
        false,
    );

    assert_eq!(run.result, Ok(()));
    // The free bytes of OSBYTE 128: 15 for an empty queue, 3 for each note
    assert_eq!(run.text, "12 9 15 15\n");
}

#[test]
fn adval_of_a_sound_queue_before_any_sound_gives_15() {
    let (result, output) = run_with_limit(&["10 PRINT STR$(ADVAL(-5)) \" \" STR$(ADVAL(-8))"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "15 15\n");
}

#[test]
fn escape_stops_a_wait_and_empties_the_queues() {
    let run = run_sound(
        &[
            "10 SOUND 1, -15, 53, 10",
            "20 SOUND 0, -15, 4, 10",
            "30 FOR I% = 1 TO 6 : SOUND 2, -15, 53, 10 : NEXT",
            "40 PRINT \"not here\"",
        ],
        true,
    );

    assert_eq!(run.result, Err("Escape at line 30".to_string()));
    assert_eq!(run.text, "");
    let engine = lock(&run.engine);
    assert!((0..4).all(|channel| engine.free_space(channel) == 15));
}

#[test]
fn tempo_voice_and_voices_are_not_supported() {
    for keyword in ["TEMPO 10", "VOICE 1, \"x\"", "VOICES 4"] {
        let (result, _) = run_with_limit(&[&format!("10 {keyword}")]);
        assert_eq!(
            result,
            Err("Syntax error at line 10: Not supported on the BBC Micro".to_string()),
            "{keyword}"
        );
    }
}

#[test]
fn the_binary_runs_a_tune_and_continues_while_it_plays() {
    let output = run_binary_read_first(
        "10 SOUND 1, -15, 53, 4 : SOUND 1, -15, 69, 4\n\
         20 PRINT \"playing\"; ADVAL(-6)\n\
         RUN\n\
         EXIT\n",
    );

    assert!(output.contains("playing"), "{output}");
    assert!(!output.contains("error"), "{output}");
}
