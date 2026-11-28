//! Issue #209: the ENVELOPE statement, and SOUND with an envelope number.
//! The rules are those of MOS 1.20 (see `src/sound/envelope.rs`).
//!
//! A fake output takes no samples. The test advances the clock, so the
//! timing is exact.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::parser::{parse_expression, parse_statement, Statement};
use bbc_basic_interpreter::program::ProgramStore;
use bbc_basic_interpreter::runner::run_program;
use bbc_basic_interpreter::sound::{lock, SharedEngine, SoundOutput};
use bbc_basic_interpreter::tokenizer::tokenize;

use crate::common::{run_binary_read_first, run_with_limit};

/// The samples of one tick (1/100 second) at 44100 samples each second.
const TICK: usize = 441;

/// An output that takes no samples.
struct StillOutput;

impl SoundOutput for StillOutput {}

/// Run the program with an output that takes no samples, on a thread with
/// a time limit of 5 seconds. Give the result and the engine.
fn run_sound(source: &[&str]) -> (Result<(), String>, SharedEngine) {
    let lines: Vec<String> = source.iter().map(|line| line.to_string()).collect();
    let slot: Arc<Mutex<Option<SharedEngine>>> = Arc::new(Mutex::new(None));
    let factory_slot = Arc::clone(&slot);
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut executor = Executor::new();
        executor.set_sound_output(Box::new(move |engine: SharedEngine| {
            lock(&engine).record_notes();
            *factory_slot.lock().unwrap() = Some(engine);
            Box::new(StillOutput) as Box<dyn SoundOutput>
        }));
        let mut program = ProgramStore::new();
        for line in &lines {
            program.store_line(tokenize(line).expect("line tokenizes"));
        }
        let _ = sender.send(run_program(&mut executor, &mut program));
    });
    let result = receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("the program stops in 5 seconds");
    let engine = slot
        .lock()
        .unwrap()
        .clone()
        .expect("the program made an output");
    (result, engine)
}

/// The (level, pitch) of `channel` after each of `ticks` ticks.
fn shape(engine: &SharedEngine, channel: u8, ticks: usize) -> Vec<(u8, u8)> {
    let mut buffer = vec![0.0f32; TICK];
    (0..ticks)
        .map(|_| {
            let mut engine = lock(engine);
            engine.fill(&mut buffer);
            (engine.level(channel), engine.pitch(channel))
        })
        .collect()
}

/// The expression of a source text. A PRINT in front of it stops the
/// tokenizer from reading a number at the start as a line number.
fn expression(source: &str) -> bbc_basic_interpreter::Expression {
    parse_expression(&tokenize(&format!("PRINT {source}")).unwrap().tokens[1..]).unwrap()
}

#[test]
fn envelope_parses_14_values() {
    let source = "1, 2, 3, -4, 5, 6, 7, 8, 9, -10, 11, -12, 126, N%+1";
    let statement = parse_statement(&tokenize(&format!("ENVELOPE {source}")).unwrap()).unwrap();

    let values: Vec<_> = source.split(", ").map(expression).collect();
    assert_eq!(
        statement,
        Statement::Envelope {
            values: Box::new(values.try_into().unwrap()),
        }
    );
}

#[test]
fn envelope_with_a_wrong_count_of_values_is_a_syntax_error() {
    for (line, count) in [
        ("10 ENVELOPE", 0),
        ("10 ENVELOPE 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0", 13),
        (
            "10 ENVELOPE 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0",
            15,
        ),
    ] {
        let (result, _) = run_with_limit(&[line]);
        assert_eq!(
            result,
            Err(format!(
                "Syntax error at line 10: ENVELOPE requires 14 parameters \
                 (N, T, PI1, PI2, PI3, PN1, PN2, PN3, AA, AD, AS, AR, ALA, ALD), got {count}"
            )),
            "{line}"
        );
    }
}

#[test]
fn a_sound_with_envelope_1_follows_envelope_1() {
    // A pitch sweep up of 4 at each step for 10 steps, an attack of 32 to
    // ALA = 126, a decay of -8 to ALD = 63, and a release of -16
    let (result, engine) = run_sound(&[
        "10 ENVELOPE 1, 1, 4, 0, 0, 10, 0, 0, 32, -8, 0, -16, 126, 63",
        "20 SOUND 1, 1, 100, 3",
    ]);
    assert_eq!(result, Ok(()));

    let found = shape(&engine, 1, 16);

    // Attack: 4, 8, 12, then 15 (ALA). Decay to level 8 (ALD). The pitch
    // goes up for 10 steps. Sections 2 and 3 have no steps and take one
    // step each, then the auto-repeat (bit 7 of T clear) starts again from
    // the base pitch. The duration of 3 ends at tick 14, and the release
    // starts there.
    assert_eq!(
        found,
        vec![
            (4, 104),
            (8, 108),
            (12, 112),
            (15, 116),
            (14, 120),
            (13, 124),
            (12, 128),
            (11, 132),
            (10, 136),
            (9, 140),
            (8, 140),
            (8, 140),
            (8, 104),
            (8, 108),
            (6, 112),
            (4, 116)
        ]
    );
}

#[test]
fn a_sound_with_no_envelope_does_not_change() {
    let (result, engine) = run_sound(&[
        "10 ENVELOPE 1, 1, 4, 0, 0, 10, 0, 0, 32, -8, 0, -16, 126, 63",
        "20 SOUND 2, -10, 100, 3",
    ]);
    assert_eq!(result, Ok(()));

    assert_eq!(shape(&engine, 2, 5), vec![(10, 100); 5]);
}

#[test]
fn the_values_use_their_low_bytes() {
    // N = 257 is envelope 1, T = 257 is 1, PI1 = 260 is 4
    let (result, engine) = run_sound(&[
        "10 ENVELOPE 257, 257, 260, 0, 0, 10, 0, 0, 32, -8, 0, -16, 126, 63",
        "20 SOUND 1, 1, 100, 3",
    ]);
    assert_eq!(result, Ok(()));

    assert_eq!(shape(&engine, 1, 2), vec![(4, 104), (8, 108)]);
}

#[test]
fn a_variable_and_an_array_named_envelope_still_work() {
    let (result, output) = run_with_limit(&[
        "10 envelope = 3 : DIM envelope(2) : envelope(1) = 7",
        "20 PRINT envelope; \" \"; envelope(1)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         3 7\n");
}

#[test]
fn the_binary_runs_envelope_and_sound() {
    let output = run_binary_read_first(
        "10 ENVELOPE 1, 1, 4, 0, 0, 10, 0, 0, 32, -8, 0, -16, 126, 63\n\
         20 SOUND 1, 1, 100, 3\n\
         30 PRINT \"shaped\"\n\
         RUN\n\
         EXIT\n",
    );

    assert!(output.contains("shaped"), "{output}");
    assert!(!output.contains("rror"), "{output}");
}
