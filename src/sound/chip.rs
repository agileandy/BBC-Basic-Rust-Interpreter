//! A model of the SN76489 sound chip of the BBC Micro Model B: 3 tone
//! channels, 1 noise channel and a mixer to one mono stream.
//!
//! The model steps the chip at its internal rate (the clock divided by 16)
//! and gives the mean output of the steps in each sample. It has no clock,
//! no thread and no audio device, so a render is deterministic.

use super::pitch::{effective_divider, CHIP_CLOCK_HZ};

/// The rate in Hz at which the counters of the chip step: 4 MHz / 16.
pub const CHIP_TICK_HZ: u32 = CHIP_CLOCK_HZ / 16;

/// The sample rate of `Chip::default()`, in Hz.
pub const DEFAULT_SAMPLE_RATE: u32 = 44_100;

/// The value of the noise shift register after a write to the noise register.
const NOISE_SEED: u16 = 0x4000;

/// The half periods in ticks of the 3 fixed noise rates (settings 0 to 2).
const NOISE_HALF_PERIOD: [u16; 3] = [16, 32, 64];

/// The amplitude of a level (0 to 15). Level 15 is 1.0 and each step below
/// it is 2 dB less. Level 0 is silence.
fn amplitude(level: u8) -> f32 {
    if level == 0 {
        0.0
    } else {
        10f32.powf(-f32::from(15 - level) / 10.0)
    }
}

/// A counter that inverts a square wave each `half_period` ticks.
#[derive(Debug, Clone, Default)]
struct Square {
    half_period: u16,
    counter: u16,
    high: bool,
}

impl Square {
    /// Steps the counter by one tick. Gives true if the wave goes high.
    fn tick(&mut self) -> bool {
        if self.counter > 1 {
            self.counter -= 1;
            false
        } else {
            self.counter = self.half_period;
            self.high = !self.high;
            self.high
        }
    }

    fn sign(&self) -> f32 {
        if self.high {
            1.0
        } else {
            -1.0
        }
    }
}

#[derive(Debug, Clone, Default)]
struct Tone {
    wave: Square,
    level: u8,
}

#[derive(Debug, Clone)]
struct Noise {
    setting: u8,
    level: u8,
    clock: Square,
    register: u16,
}

impl Noise {
    /// Steps the noise channel by one tick. `tone_1_high` is true if the
    /// wave of SOUND channel 1 went high on this tick.
    fn tick(&mut self, tone_1_high: bool) {
        let shift = if self.setting & 3 == 3 {
            tone_1_high
        } else {
            self.clock.tick()
        };
        if shift {
            let feedback = if self.setting & 4 != 0 {
                // White noise: the taps are bits 0 and 1.
                (self.register ^ (self.register >> 1)) & 1
            } else {
                // Periodic noise: bit 0 goes back to bit 14.
                self.register & 1
            };
            self.register = (self.register >> 1) | (feedback << 14);
        }
    }

    fn sign(&self) -> f32 {
        if self.register & 1 != 0 {
            1.0
        } else {
            -1.0
        }
    }
}

/// The SN76489 sound chip, with the channel numbers of the SOUND statement.
///
/// Channels 1, 2 and 3 are tone channels. Channel 0 is the noise channel.
/// A new chip is silent: each level is 0.
#[derive(Debug, Clone)]
pub struct Chip {
    sample_rate: u32,
    /// The tone channels 1, 2 and 3, at index 0, 1 and 2.
    tones: [Tone; 3],
    noise: Noise,
    /// The ticks that are left over from the last sample, in units of
    /// 1 / sample rate of a tick.
    remainder: u32,
}

impl Default for Chip {
    /// A silent chip at a sample rate of 44100 Hz.
    fn default() -> Self {
        Self::new(DEFAULT_SAMPLE_RATE)
    }
}

impl Chip {
    /// Makes a silent chip that renders at `sample_rate` samples each second.
    ///
    /// # Panics
    ///
    /// Panics if `sample_rate` is 0.
    pub fn new(sample_rate: u32) -> Self {
        assert!(sample_rate > 0, "sample rate must be more than 0");
        Self {
            sample_rate,
            tones: Default::default(),
            noise: Noise {
                setting: 0,
                level: 0,
                clock: Square {
                    half_period: NOISE_HALF_PERIOD[0],
                    ..Square::default()
                },
                register: NOISE_SEED,
            },
            remainder: 0,
        }
    }

    /// Gives the sample rate in Hz that `render` uses.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Sets the 10-bit `divider` and the `level` of tone `channel` (1 to 3).
    ///
    /// The frequency is 4 MHz / (32 x divider), and a divider of 0 is 1024.
    /// Use `sound::divider` to get the divider of a SOUND pitch value. The
    /// level is 0 (silence) to 15 (the loudest), with 2 dB for each step.
    /// The SOUND statement uses -15 for the loudest: its caller converts the
    /// value. A new divider does not reset the wave, as on the chip.
    ///
    /// # Panics
    ///
    /// Panics if `channel` is not 1 to 3, `divider` is more than 1023 or
    /// `level` is more than 15.
    pub fn set_tone(&mut self, channel: u8, divider: u16, level: u8) {
        assert!(
            (1..=3).contains(&channel),
            "tone channel must be 1 to 3, not {channel}"
        );
        assert!(divider <= 0x3FF, "divider must be 0 to 1023, not {divider}");
        assert!(level <= 15, "level must be 0 to 15, not {level}");
        let tone = &mut self.tones[usize::from(channel - 1)];
        tone.wave.half_period = effective_divider(divider);
        tone.level = level;
    }

    /// Sets the noise `setting` (0 to 7) and the `level` of channel 0.
    ///
    /// Settings 0 to 3 are periodic noise and settings 4 to 7 are white
    /// noise. The rate is 4 MHz / 512, / 1024 or / 2048 for settings 0, 1
    /// and 2 (and 4, 5 and 6). Settings 3 and 7 shift the noise once for
    /// each wave of tone channel 1. As on the chip, each call resets the
    /// noise shift register. The level is as for `set_tone`.
    ///
    /// # Panics
    ///
    /// Panics if `setting` is more than 7 or `level` is more than 15.
    pub fn set_noise(&mut self, setting: u8, level: u8) {
        assert!(setting <= 7, "noise setting must be 0 to 7, not {setting}");
        assert!(level <= 15, "level must be 0 to 15, not {level}");
        self.noise.setting = setting;
        self.noise.level = level;
        self.noise.register = NOISE_SEED;
        if setting & 3 != 3 {
            self.noise.clock.half_period = NOISE_HALF_PERIOD[usize::from(setting & 3)];
        }
    }

    /// Sets only the `level` (0 to 15) of `channel` (0 to 3). The divider,
    /// the noise setting and the waves do not change.
    ///
    /// # Panics
    ///
    /// Panics if `channel` is more than 3 or `level` is more than 15.
    pub fn set_level(&mut self, channel: u8, level: u8) {
        assert!(channel <= 3, "channel must be 0 to 3, not {channel}");
        assert!(level <= 15, "level must be 0 to 15, not {level}");
        match channel {
            0 => self.noise.level = level,
            c => self.tones[usize::from(c - 1)].level = level,
        }
    }

    /// Fills `samples` with the mono output of the chip, from -1.0 to 1.0,
    /// and advances the chip. A second call continues the waves with no
    /// step. Each of the 4 channels gives a maximum of a quarter of the
    /// full scale, so the sum does not clip.
    pub fn render(&mut self, samples: &mut [f32]) {
        for sample in samples.iter_mut() {
            self.remainder += CHIP_TICK_HZ;
            let mut sum = 0.0f32;
            let mut ticks = 0u32;
            while self.remainder >= self.sample_rate {
                self.remainder -= self.sample_rate;
                self.tick();
                sum += self.mix();
                ticks += 1;
            }
            *sample = if ticks == 0 {
                self.mix()
            } else {
                sum / ticks as f32
            };
        }
    }

    fn tick(&mut self) {
        let mut tone_1_high = false;
        for (index, tone) in self.tones.iter_mut().enumerate() {
            let high = tone.wave.tick();
            if index == 0 {
                tone_1_high = high;
            }
        }
        self.noise.tick(tone_1_high);
    }

    fn mix(&self) -> f32 {
        let tones: f32 = self
            .tones
            .iter()
            .map(|t| t.wave.sign() * amplitude(t.level))
            .sum();
        (tones + self.noise.sign() * amplitude(self.noise.level)) / 4.0
    }
}
