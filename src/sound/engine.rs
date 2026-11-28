//! The sound engine: the queues of the SOUND statement and their clock, in
//! front of the SN76489 model.
//!
//! The clock is the count of samples that the output took with `fill`, not
//! the wall clock. A recording to a file and a test thus get the same timing
//! as the speakers.

use super::chip::{Chip, DEFAULT_SAMPLE_RATE};

/// The engine owns the chip and the queues. An output takes its samples
/// with `fill`.
#[derive(Debug, Clone)]
pub struct SoundEngine {
    chip: Chip,
    /// The count of samples that `fill` gave
    clock: u64,
}

impl Default for SoundEngine {
    fn default() -> Self {
        Self::new(DEFAULT_SAMPLE_RATE)
    }
}

impl SoundEngine {
    /// An engine that makes `sample_rate` samples each second. All channels
    /// are silent and all queues are empty.
    pub fn new(sample_rate: u32) -> Self {
        Self {
            chip: Chip::new(sample_rate),
            clock: 0,
        }
    }

    /// The samples each second.
    pub fn sample_rate(&self) -> u32 {
        self.chip.sample_rate()
    }

    /// Use the sample rate of the output. The output calls this before its
    /// first `fill`.
    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        self.chip = Chip::new(sample_rate);
    }

    /// The count of samples that `fill` gave since the engine started.
    pub fn clock(&self) -> u64 {
        self.clock
    }

    /// Fill `buffer` with the next samples (-1.0 to 1.0) and advance the
    /// clock by its length. The call does not wait.
    pub fn fill(&mut self, buffer: &mut [f32]) {
        self.chip.render(buffer);
        self.clock += buffer.len() as u64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_engine_gives_silence_and_counts_the_samples() {
        let mut engine = SoundEngine::default();
        let mut buffer = vec![1.0f32; 441];

        engine.fill(&mut buffer);
        engine.fill(&mut buffer[..100]);

        assert_eq!(engine.sample_rate(), 44_100);
        assert_eq!(engine.clock(), 541);
        assert!(buffer.iter().all(|s| *s == 0.0));
    }
}
