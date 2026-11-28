//! A recording of the sound of a run to a WAV file (issue #211): 16-bit
//! mono PCM at the sample rate of the engine, with no new dependency.
//!
//! The format is the RIFF WAVE file with one `fmt ` chunk (PCM, format 1)
//! and one `data` chunk. The header has two sizes: the RIFF size at byte 4
//! and the data size at byte 40. The writer puts 0 there at the start, and
//! writes the true sizes after each second of samples and at the end. A
//! file of a run that stops early thus has a valid header.

use std::fs::File;
use std::io::{self, BufWriter, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// The bytes of the header before the samples.
const HEADER_BYTES: u32 = 44;
/// The bytes of one sample: 16-bit mono.
const SAMPLE_BYTES: u32 = 2;
/// The place of the RIFF size and of the data size in the header.
const RIFF_SIZE_AT: u64 = 4;
const DATA_SIZE_AT: u64 = 40;
const SAMPLE_RATE_AT: u64 = 24;

/// Writes a WAV file of 16-bit mono samples to `out`.
#[derive(Debug)]
pub struct WavWriter<W: Write + Seek> {
    out: W,
    sample_rate: u32,
    /// The samples written
    samples: u32,
    /// The samples written since the last write of the sizes
    since_sizes: u32,
}

impl<W: Write + Seek> WavWriter<W> {
    /// Write the header, with the sizes 0, at the start of `out`.
    pub fn new(mut out: W, sample_rate: u32) -> io::Result<Self> {
        let mut header = Vec::with_capacity(HEADER_BYTES as usize);
        header.extend_from_slice(b"RIFF");
        header.extend_from_slice(&0u32.to_le_bytes());
        header.extend_from_slice(b"WAVEfmt ");
        header.extend_from_slice(&16u32.to_le_bytes());
        header.extend_from_slice(&1u16.to_le_bytes()); // PCM
        header.extend_from_slice(&1u16.to_le_bytes()); // mono
        header.extend_from_slice(&sample_rate.to_le_bytes());
        header.extend_from_slice(&(sample_rate * SAMPLE_BYTES).to_le_bytes());
        header.extend_from_slice(&(SAMPLE_BYTES as u16).to_le_bytes());
        header.extend_from_slice(&16u16.to_le_bytes());
        header.extend_from_slice(b"data");
        header.extend_from_slice(&0u32.to_le_bytes());
        out.write_all(&header)?;
        Ok(Self {
            out,
            sample_rate,
            samples: 0,
            since_sizes: 0,
        })
    }

    /// Use another sample rate. This has an effect only before the first
    /// sample.
    pub fn set_sample_rate(&mut self, sample_rate: u32) -> io::Result<()> {
        if self.samples > 0 || sample_rate == self.sample_rate {
            return Ok(());
        }
        self.sample_rate = sample_rate;
        self.out.seek(SeekFrom::Start(SAMPLE_RATE_AT))?;
        self.out.write_all(&sample_rate.to_le_bytes())?;
        self.out
            .write_all(&(sample_rate * SAMPLE_BYTES).to_le_bytes())?;
        self.out.seek(SeekFrom::End(0))?;
        Ok(())
    }

    /// Write samples of -1.0 to 1.0. A value out of this range clips.
    pub fn write(&mut self, samples: &[f32]) -> io::Result<()> {
        let mut bytes = Vec::with_capacity(samples.len() * SAMPLE_BYTES as usize);
        for sample in samples {
            let value = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        self.out.write_all(&bytes)?;
        let count = samples.len() as u32;
        self.samples = self.samples.saturating_add(count);
        self.since_sizes = self.since_sizes.saturating_add(count);
        if self.since_sizes >= self.sample_rate {
            self.write_sizes()?;
        }
        Ok(())
    }

    /// The count of samples written.
    pub fn sample_count(&self) -> u32 {
        self.samples
    }

    /// Write the sizes and give `out` back.
    pub fn finish(mut self) -> io::Result<W> {
        self.write_sizes()?;
        self.out.flush()?;
        Ok(self.out)
    }

    /// Write the true sizes in the header, then go back to the end.
    fn write_sizes(&mut self) -> io::Result<()> {
        let data = self.samples.saturating_mul(SAMPLE_BYTES);
        self.out.seek(SeekFrom::Start(RIFF_SIZE_AT))?;
        self.out
            .write_all(&data.saturating_add(HEADER_BYTES - 8).to_le_bytes())?;
        self.out.seek(SeekFrom::Start(DATA_SIZE_AT))?;
        self.out.write_all(&data.to_le_bytes())?;
        self.out.seek(SeekFrom::End(0))?;
        self.since_sizes = 0;
        Ok(())
    }
}

/// The state of a recorder: the writer until `finish`, and the first error.
#[derive(Debug)]
struct RecorderState {
    writer: Option<WavWriter<BufWriter<File>>>,
    error: Option<io::Error>,
}

impl Drop for RecorderState {
    /// Write the sizes if no `finish` came, for example after a panic.
    fn drop(&mut self) {
        if let Some(writer) = self.writer.take() {
            let _ = writer.finish();
        }
    }
}

/// A recording to a WAV file that the engine and the front end share. The
/// engine gives it each sample that an output takes with `fill`.
#[derive(Debug, Clone)]
pub struct WavRecorder {
    state: Arc<Mutex<RecorderState>>,
}

impl WavRecorder {
    /// Create the file at `path`, with the sample rate of a new engine. The
    /// engine gives its true rate with the first samples.
    pub fn create(path: &Path) -> io::Result<Self> {
        let file = BufWriter::new(File::create(path)?);
        let writer = WavWriter::new(file, super::DEFAULT_SAMPLE_RATE)?;
        Ok(Self {
            state: Arc::new(Mutex::new(RecorderState {
                writer: Some(writer),
                error: None,
            })),
        })
    }

    /// Write `samples` that the engine made at `sample_rate`. After an
    /// error, the recorder keeps the error for `finish` and writes nothing.
    pub fn write(&self, samples: &[f32], sample_rate: u32) {
        let mut state = self.lock();
        let Some(writer) = state.writer.as_mut() else {
            return;
        };
        let result = writer
            .set_sample_rate(sample_rate)
            .and_then(|()| writer.write(samples));
        if let Err(error) = result {
            state.writer = None;
            state.error = Some(error);
        }
    }

    /// Write the sizes and close the file. Give the first error of the
    /// recording. A second call does nothing.
    pub fn finish(&self) -> io::Result<()> {
        let mut state = self.lock();
        if let Some(error) = state.error.take() {
            return Err(error);
        }
        match state.writer.take() {
            Some(writer) => writer.finish().map(drop),
            None => Ok(()),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, RecorderState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn u16_at(bytes: &[u8], at: usize) -> u16 {
        u16::from_le_bytes([bytes[at], bytes[at + 1]])
    }

    fn u32_at(bytes: &[u8], at: usize) -> u32 {
        u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
    }

    #[test]
    fn the_header_is_a_16_bit_mono_pcm_header_with_the_sizes() {
        let mut writer = WavWriter::new(Cursor::new(Vec::new()), 22_050).unwrap();
        writer.write(&[0.0, 1.0, -1.0, 0.5]).unwrap();
        let bytes = writer.finish().unwrap().into_inner();

        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(u32_at(&bytes, 4), 36 + 8, "RIFF size");
        assert_eq!(&bytes[8..16], b"WAVEfmt ");
        assert_eq!(u32_at(&bytes, 16), 16, "fmt chunk size");
        assert_eq!(u16_at(&bytes, 20), 1, "PCM");
        assert_eq!(u16_at(&bytes, 22), 1, "mono");
        assert_eq!(u32_at(&bytes, 24), 22_050, "sample rate");
        assert_eq!(u32_at(&bytes, 28), 44_100, "bytes each second");
        assert_eq!(u16_at(&bytes, 32), 2, "block align");
        assert_eq!(u16_at(&bytes, 34), 16, "bits each sample");
        assert_eq!(&bytes[36..40], b"data");
        assert_eq!(u32_at(&bytes, 40), 8, "data size");
        assert_eq!(bytes.len(), 44 + 8);
    }

    #[test]
    fn the_samples_are_16_bit_values_and_clip_at_full_scale() {
        let mut writer = WavWriter::new(Cursor::new(Vec::new()), 8_000).unwrap();
        writer.write(&[0.0, 1.0, -1.0, 0.5, 2.0, -3.0]).unwrap();
        let bytes = writer.finish().unwrap().into_inner();

        let samples: Vec<i16> = bytes[44..]
            .chunks(2)
            .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        assert_eq!(samples, vec![0, 32_767, -32_767, 16_384, 32_767, -32_767]);
    }

    #[test]
    fn the_header_has_the_sizes_after_each_second_with_no_finish() {
        let mut writer = WavWriter::new(Cursor::new(Vec::new()), 100).unwrap();
        writer.write(&[0.25; 150]).unwrap();
        let bytes = writer.out.get_ref().clone();

        // 100 samples is one second: the sizes are those of the first second
        // or more, and the samples continue after the header
        assert!(
            u32_at(&bytes, 40) >= 200,
            "data size {}",
            u32_at(&bytes, 40)
        );
        assert_eq!(bytes.len(), 44 + 300);
    }

    #[test]
    fn the_sample_rate_can_change_before_the_first_sample() {
        let mut writer = WavWriter::new(Cursor::new(Vec::new()), 44_100).unwrap();
        writer.set_sample_rate(48_000).unwrap();
        writer.write(&[0.0]).unwrap();
        let bytes = writer.finish().unwrap().into_inner();

        assert_eq!(u32_at(&bytes, 24), 48_000);
        assert_eq!(u32_at(&bytes, 28), 96_000);
    }

    #[test]
    fn a_recorder_writes_a_file_with_the_samples_of_the_engine() {
        let path =
            std::env::temp_dir().join(format!("bbc_basic_wav_recorder_{}.wav", std::process::id()));
        let recorder = WavRecorder::create(&path).unwrap();
        let mut engine = crate::sound::SoundEngine::new(10_000);
        engine.record_samples(recorder.clone());
        assert!(engine.try_sound(1, -15, 89, 1));

        let mut buffer = vec![0.0f32; 2_500];
        engine.fill(&mut buffer);
        recorder.finish().unwrap();

        let bytes = std::fs::read(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(u32_at(&bytes, 24), 10_000);
        assert_eq!(u32_at(&bytes, 40), 5_000, "2500 samples of 2 bytes");
        let written: Vec<f32> = bytes[44..]
            .chunks(2)
            .map(|pair| f32::from(i16::from_le_bytes([pair[0], pair[1]])) / 32_767.0)
            .collect();
        let expected: Vec<f32> = buffer.iter().map(|s| s.clamp(-1.0, 1.0)).collect();
        assert!(written.iter().any(|s| *s != 0.0), "the note is in the file");
        assert!(written
            .iter()
            .zip(&expected)
            .all(|(w, e)| (w - e).abs() < 0.001));
    }
}
