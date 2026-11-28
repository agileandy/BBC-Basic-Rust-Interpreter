//! The seam between the sound engine and an audio output (issues #208 and
//! #210).
//!
//! The engine owns the chip and the queues. An output pulls the samples with
//! `SoundEngine::fill` on the shared engine, so the clock of the queues is the
//! count of samples that the output took. The executor makes the output at
//! the first SOUND, with the factory that a front end gave. With no factory,
//! it uses `ClockOutput`, which takes the samples in real time and discards
//! them.

use super::engine::SoundEngine;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// The engine, shared between the executor and the thread of an output. A
/// lock must be short: `fill` does not wait.
pub type SharedEngine = Arc<Mutex<SoundEngine>>;

/// An audio output. It takes samples from a `SharedEngine` on its own
/// thread or callback.
pub trait SoundOutput: Send {
    /// Stop taking samples. The default does nothing.
    fn stop(&mut self) {}
}

/// Makes the output for an engine. The executor calls it at the first SOUND.
pub type OutputFactory = Box<dyn FnOnce(SharedEngine) -> Box<dyn SoundOutput> + Send>;

/// Lock the engine. A panic on another thread does not stop the sound.
pub fn lock(engine: &SharedEngine) -> std::sync::MutexGuard<'_, SoundEngine> {
    engine
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The time between two takes of `ClockOutput`.
const CLOCK_STEP: Duration = Duration::from_millis(5);

/// An output with no device (contract C3 of issue #208). A small thread
/// takes the samples in real time and discards them, so that the notes end
/// and a SOUND to a full queue waits as with a device.
pub struct ClockOutput {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl ClockOutput {
    /// Start the thread.
    pub fn start(engine: SharedEngine) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let thread = thread::spawn(move || {
            let start = Instant::now();
            let first = lock(&engine).clock();
            let mut scratch = vec![0.0f32; 4096];
            while !flag.load(Ordering::SeqCst) {
                {
                    let mut engine = lock(&engine);
                    let due = first
                        + (start.elapsed().as_secs_f64() * f64::from(engine.sample_rate())) as u64;
                    while engine.clock() < due {
                        let count = (due - engine.clock()).min(scratch.len() as u64) as usize;
                        engine.fill(&mut scratch[..count]);
                    }
                }
                thread::sleep(CLOCK_STEP);
            }
        });
        Self {
            stop,
            thread: Some(thread),
        }
    }
}

impl SoundOutput for ClockOutput {
    fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for ClockOutput {
    fn drop(&mut self) {
        SoundOutput::stop(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clock_output_takes_samples_in_real_time() {
        let engine: SharedEngine = Arc::new(Mutex::new(SoundEngine::new(10_000)));
        let mut output = ClockOutput::start(Arc::clone(&engine));

        thread::sleep(Duration::from_millis(100));
        output.stop();
        let taken = lock(&engine).clock();
        thread::sleep(Duration::from_millis(20));

        // 100 ms at 10000 samples each second is 1000 samples
        assert!((900..=3000).contains(&taken), "took {taken} samples");
        assert_eq!(lock(&engine).clock(), taken, "no samples after stop");
    }
}
