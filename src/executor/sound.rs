//! The SOUND statement and the sound queues of ADVAL (issue #208), and the
//! ENVELOPE statement (issue #209).
//!
//! The executor makes the sound engine and its output at the first SOUND or
//! ENVELOPE, because the engine keeps the envelopes.
//! The output comes from the factory of a front end, or is a `ClockOutput`
//! with no device. The notes continue after the program ends, as on a BBC
//! Micro: RUN, NEW and END do not flush the queues (BASIC 2 has no call to
//! flush them). An Escape flushes them, as the MOS does when BASIC
//! acknowledges the Escape (OSBYTE 126, MOS 1.20 chapter 15, section 4).

use super::Executor;
use crate::error::{BBCBasicError, Result};
use crate::parser::Expression;
use crate::sound::{
    lock, ClockOutput, OutputFactory, SharedEngine, SoundEngine, SoundOutput, WavRecorder,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The time between two tries of a SOUND to a full queue.
const WAIT_STEP: Duration = Duration::from_millis(2);

/// The wait for the end of the sound stops if the output takes no samples
/// for this time (issue #211).
const STALL_LIMIT: Duration = Duration::from_secs(1);

/// The free bytes of an empty queue, for ADVAL before the first SOUND.
const EMPTY_QUEUE: i32 = 15;

/// The sound of an executor: the engine and its output after the first
/// SOUND, and the factory of a front end before it.
#[derive(Default)]
pub(super) struct SoundSlot {
    engine: Option<SharedEngine>,
    output: Option<Box<dyn SoundOutput>>,
    factory: Option<OutputFactory>,
    /// The recording to a WAV file, for each engine (issue #211)
    recorder: Option<WavRecorder>,
}

impl std::fmt::Debug for SoundSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoundSlot")
            .field("started", &self.engine.is_some())
            .field("factory", &self.factory.is_some())
            .field("recorder", &self.recorder.is_some())
            .finish()
    }
}

impl Executor {
    /// Give the factory of the audio output. The executor calls it at the
    /// first SOUND. If the sound started, the new output replaces the old one.
    pub fn set_sound_output(&mut self, factory: OutputFactory) {
        match self.sound.engine.clone() {
            Some(engine) => {
                self.stop_output();
                self.sound.output = Some(factory(engine));
            }
            None => self.sound.factory = Some(factory),
        }
    }

    /// Stop the audio output, for example before a front end closes. A later
    /// SOUND starts a new engine with no device.
    pub fn stop_sound(&mut self) {
        self.stop_output();
        self.sound.engine = None;
    }

    /// Record each sample of the sound to `recorder` (issue #211). The
    /// recording takes the samples that the output takes, so its timing is
    /// the timing of the output.
    pub fn record_sound(&mut self, recorder: WavRecorder) {
        if let Some(engine) = &self.sound.engine {
            lock(engine).record_samples(recorder.clone());
        }
        self.sound.recorder = Some(recorder);
    }

    /// Wait until no sound will change (`SoundEngine::is_idle`), for a
    /// maximum of `limit`. Before the first SOUND, there is no wait. If the
    /// output takes no samples for `STALL_LIMIT`, the sound cannot end, so
    /// the wait stops.
    pub fn wait_for_sound(&mut self, limit: Duration) {
        let Some(engine) = self.sound.engine.clone() else {
            return;
        };
        let now = std::time::Instant::now;
        let deadline = now() + limit;
        let mut clock = lock(&engine).clock();
        let mut last_change = now();
        loop {
            {
                let engine = lock(&engine);
                if engine.is_idle() {
                    return;
                }
                if engine.clock() != clock {
                    clock = engine.clock();
                    last_change = now();
                }
            }
            if now() >= deadline || last_change.elapsed() >= STALL_LIMIT {
                return;
            }
            std::thread::sleep(WAIT_STEP);
        }
    }

    /// Empty all sound queues and silence all channels (Escape).
    pub fn flush_sound(&mut self) {
        if let Some(engine) = &self.sound.engine {
            lock(engine).flush();
        }
    }

    fn stop_output(&mut self) {
        if let Some(mut output) = self.sound.output.take() {
            output.stop();
        }
    }

    /// The engine, which starts with its output at the first call.
    fn sound_engine(&mut self) -> SharedEngine {
        if let Some(engine) = &self.sound.engine {
            return Arc::clone(engine);
        }
        let mut new_engine = SoundEngine::default();
        if let Some(recorder) = &self.sound.recorder {
            new_engine.record_samples(recorder.clone());
        }
        let engine: SharedEngine = Arc::new(Mutex::new(new_engine));
        let output = match self.sound.factory.take() {
            Some(factory) => factory(Arc::clone(&engine)),
            None => Box::new(ClockOutput::start(Arc::clone(&engine))),
        };
        self.sound.output = Some(output);
        self.sound.engine = Some(Arc::clone(&engine));
        engine
    }

    /// SOUND channel, amplitude, pitch, duration. A SOUND to a full queue
    /// waits until a place is free, and an Escape stops the wait.
    pub(super) fn execute_sound(
        &mut self,
        channel: &Expression,
        amplitude: &Expression,
        pitch: &Expression,
        duration: &Expression,
    ) -> Result<()> {
        let channel = self.eval_integer(channel)?;
        let amplitude = self.eval_integer(amplitude)?;
        let pitch = self.eval_integer(pitch)?;
        let duration = self.eval_integer(duration)?;
        let engine = self.sound_engine();
        loop {
            {
                let mut engine = lock(&engine);
                if engine.try_sound(channel, amplitude, pitch, duration) {
                    engine.set_waiting(false);
                    return Ok(());
                }
                engine.set_waiting(true);
            }
            if self.take_interrupt().is_some() {
                lock(&engine).set_waiting(false);
                return Err(BBCBasicError::Escape);
            }
            std::thread::sleep(WAIT_STEP);
        }
    }

    /// ENVELOPE N, T, PI1, PI2, PI3, PN1, PN2, PN3, AA, AD, AS, AR, ALA,
    /// ALD: keep envelope N in the engine, as OSWORD 8 does.
    pub(super) fn execute_envelope(&mut self, values: &[Expression; 14]) -> Result<()> {
        let mut numbers = [0i32; 14];
        for (number, value) in numbers.iter_mut().zip(values) {
            *number = self.eval_integer(value)?;
        }
        let [envelope, shape @ ..] = numbers;
        lock(&self.sound_engine()).set_envelope(envelope, shape);
        Ok(())
    }

    /// Tell if `name(...)` is a call of NOTE (issue #211). An array named NOTE
    /// has priority, so that old programs do not change.
    pub(super) fn is_note_call(&self, name: &str) -> bool {
        name == "NOTE" && !self.variables.has_array(name)
    }

    /// NOTE(name$): the pitch value of SOUND for a note name such as "C4" or
    /// "F#3" (an extension, issue #211).
    pub(super) fn eval_note(&mut self, args: &[Expression]) -> Result<i32> {
        let [name] = args else {
            return Err(BBCBasicError::SyntaxError {
                message: "NOTE requires 1 argument".to_string(),
                line: None,
            });
        };
        let name = self.eval_string(name)?;
        crate::sound::map::note_pitch(&name)
            .map(i32::from)
            .map_err(BBCBasicError::BadNote)
    }

    /// ADVAL of a sound queue: the free bytes in the queue of `channel`.
    pub(super) fn sound_free_space(&self, channel: u8) -> i32 {
        self.sound.engine.as_ref().map_or(EMPTY_QUEUE, |engine| {
            i32::from(lock(engine).free_space(channel))
        })
    }
}
