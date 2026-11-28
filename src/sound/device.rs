//! The audio output of the computer: it plays the samples of the sound
//! engine on the default output device, with the `cpal` crate.
//!
//! The executor calls the output factory at the first SOUND statement, so a
//! program with no SOUND opens no device. With no device, or an error at
//! the start, the sound is silent: a one-line note goes to standard error and
//! the clock output of the engine keeps the time of the queues (issue 210).
//!
//! No test opens a device. `factory_with` takes the opener as an argument,
//! so a test gives a fake opener.

use super::{lock, ClockOutput, OutputFactory, SharedEngine, SoundOutput};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

/// A function that opens an audio output for an engine.
pub type Opener = fn(SharedEngine) -> Result<Box<dyn SoundOutput>, String>;

/// True if `factory` can open the device. Only the binary sets it, so that
/// a test of the library or of the IDE opens no device.
static DEVICE_ON: AtomicBool = AtomicBool::new(false);

/// Let `factory` open the default device. The binary calls this at the
/// start, with false for `--no-sound`.
pub fn use_device(on: bool) {
    DEVICE_ON.store(on, Ordering::SeqCst);
}

/// The output factory of the prompt and of the IDE: the default device if
/// `use_device(true)` came, else the clock output and no device call. If
/// the device does not start, the note goes to `note`.
pub fn factory(note: Box<dyn Write + Send>) -> OutputFactory {
    let no_sound = !DEVICE_ON.load(Ordering::SeqCst);
    factory_with(no_sound, open_default_device, note)
}

/// The output factory with a given opener and a given writer for the note.
pub fn factory_with(
    no_sound: bool,
    open: Opener,
    mut note: Box<dyn Write + Send>,
) -> OutputFactory {
    Box::new(move |engine: SharedEngine| -> Box<dyn SoundOutput> {
        if no_sound {
            return Box::new(ClockOutput::start(engine));
        }
        match open(engine.clone()) {
            Ok(output) => output,
            Err(reason) => {
                let _ = writeln!(note, "No sound: {reason}");
                Box::new(ClockOutput::start(engine))
            }
        }
    })
}

/// The default output device. The stream stops when the output drops.
pub struct DeviceOutput {
    stream: Option<cpal::Stream>,
}

impl SoundOutput for DeviceOutput {
    fn stop(&mut self) {
        // A drop of the stream stops the callback
        self.stream = None;
    }
}

/// Open the default output device of the default host, and play the
/// engine on it at the sample rate of the device.
pub fn open_default_device(engine: SharedEngine) -> Result<Box<dyn SoundOutput>, String> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or_else(|| "no audio output device".to_string())?;
    let supported = device.default_output_config().map_err(|e| e.to_string())?;
    let format = supported.sample_format();
    let config: StreamConfig = supported.into();
    lock(&engine).set_sample_rate(config.sample_rate);
    let stream = match format {
        SampleFormat::F32 => build::<f32>(&device, &config, engine),
        SampleFormat::F64 => build::<f64>(&device, &config, engine),
        SampleFormat::I8 => build::<i8>(&device, &config, engine),
        SampleFormat::I16 => build::<i16>(&device, &config, engine),
        SampleFormat::I24 => build::<cpal::I24>(&device, &config, engine),
        SampleFormat::I32 => build::<i32>(&device, &config, engine),
        SampleFormat::U8 => build::<u8>(&device, &config, engine),
        SampleFormat::U16 => build::<u16>(&device, &config, engine),
        SampleFormat::U32 => build::<u32>(&device, &config, engine),
        other => return Err(format!("the sample format {other} is not supported")),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(Box::new(DeviceOutput {
        stream: Some(stream),
    }))
}

/// Build a stream that converts the mono samples of the engine to the
/// sample format `T`, the same sample on each channel of a frame.
fn build<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    engine: SharedEngine,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = usize::from(config.channels.max(1));
    let mut mono: Vec<f32> = Vec::new();
    device
        .build_output_stream(
            *config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                mono.resize(data.len() / channels, 0.0);
                lock(&engine).fill(&mut mono);
                for (frame, sample) in data.chunks_mut(channels).zip(&mono) {
                    frame.fill(T::from_sample(*sample));
                }
            },
            // No text for a stream error: in the IDE, standard error writes
            // over the screen. A short gap in the sound is not an error of
            // the program.
            |_error| {},
            None,
        )
        .map_err(|e| e.to_string())
}
