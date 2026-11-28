//! Sound for BBC BASIC: a model of the SN76489 sound chip of the BBC Micro
//! Model B. The model makes a buffer of mono samples, with no audio device.
//!
//! The channel numbers are those of the SOUND statement: 1, 2 and 3 are tone
//! channels and 0 is the noise channel (BBC Micro User Guide, chapter 30,
//! Sound, and the SOUND and ENVELOPE keyword entries).
//!
//! ```
//! use bbc_basic_interpreter::sound::{divider, Chip};
//!
//! let mut chip = Chip::default(); // 44100 samples each second
//! chip.set_tone(1, divider(1, 89), 15); // pitch 89 at the loudest level
//! let mut samples = vec![0.0f32; 441];
//! chip.render(&mut samples);
//! assert!(samples.iter().all(|s| s.abs() <= 1.0));
//! ```

mod chip;
mod pitch;

pub use chip::{Chip, CHIP_TICK_HZ, DEFAULT_SAMPLE_RATE};
pub use pitch::{divider, frequency, CHIP_CLOCK_HZ};
