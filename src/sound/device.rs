//! The audio output of the computer: it plays the samples of the sound
//! engine on the default output device, with the `cpal` crate.
//!
//! The executor calls the output factory at the first SOUND statement, so a
//! program with no SOUND opens no device. With no device, or an error at
//! the start, the sound is silent: a one-line note goes to standard error and
//! the clock output of the engine keeps the time of the queues (issue 210).
//! The same occurs when a device that opened takes no samples while the
//! engine has a note, or has a stream error (issue 212).
//!
//! No test opens a device. `factory_with` takes the opener as an argument,
//! so a test gives a fake opener.

use super::{lock, ClockOutput, OutputFactory, SharedEngine, SoundOutput};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// If the device takes no samples for this time while the engine has a
/// note, the clock output takes its place (issue 212).
const DEVICE_STALL_LIMIT: Duration = Duration::from_millis(500);

/// The time between two checks of `GuardedOutput`.
const GUARD_STEP: Duration = Duration::from_millis(20);

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
            Ok(output) => Box::new(GuardedOutput::start(engine, output, note)),
            Err(reason) => {
                let _ = writeln!(note, "No sound: {reason}");
                Box::new(ClockOutput::start(engine))
            }
        }
    })
}

/// A device output with a guard thread (issue 212). If the device fails, or
/// takes no samples for `DEVICE_STALL_LIMIT` while the engine has a note,
/// the guard stops the device, writes the note one time and starts the clock
/// output, so that the notes still end.
struct GuardedOutput {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl GuardedOutput {
    fn start(
        engine: SharedEngine,
        mut device: Box<dyn SoundOutput>,
        mut note: Box<dyn Write + Send>,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let thread = thread::spawn(move || {
            let mut clock = lock(&engine).clock();
            let mut last_change = Instant::now();
            let reason = loop {
                if flag.load(Ordering::SeqCst) {
                    device.stop();
                    return;
                }
                if device.has_failed() {
                    break "the audio device stopped";
                }
                {
                    let engine = lock(&engine);
                    if engine.is_idle() || engine.clock() != clock {
                        clock = engine.clock();
                        last_change = Instant::now();
                    }
                }
                if last_change.elapsed() >= DEVICE_STALL_LIMIT {
                    break "the audio device takes no samples";
                }
                thread::sleep(GUARD_STEP);
            };
            device.stop();
            let _ = writeln!(note, "No sound: {reason}");
            let mut clock_output = ClockOutput::start(engine);
            while !flag.load(Ordering::SeqCst) {
                thread::sleep(GUARD_STEP);
            }
            clock_output.stop();
        });
        Self {
            stop,
            thread: Some(thread),
        }
    }
}

impl SoundOutput for GuardedOutput {
    fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for GuardedOutput {
    fn drop(&mut self) {
        SoundOutput::stop(self);
    }
}

/// The default output device. The stream stops when the output drops.
pub struct DeviceOutput {
    stream: Option<cpal::Stream>,
    /// True after a stream error (issue 212)
    failed: Arc<AtomicBool>,
}

impl SoundOutput for DeviceOutput {
    fn stop(&mut self) {
        // A drop of the stream stops the callback
        self.stream = None;
    }

    fn has_failed(&self) -> bool {
        self.failed.load(Ordering::SeqCst)
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
    let failed = Arc::new(AtomicBool::new(false));
    let stream = match format {
        SampleFormat::F32 => build::<f32>(&device, &config, engine, Arc::clone(&failed)),
        SampleFormat::F64 => build::<f64>(&device, &config, engine, Arc::clone(&failed)),
        SampleFormat::I8 => build::<i8>(&device, &config, engine, Arc::clone(&failed)),
        SampleFormat::I16 => build::<i16>(&device, &config, engine, Arc::clone(&failed)),
        SampleFormat::I24 => build::<cpal::I24>(&device, &config, engine, Arc::clone(&failed)),
        SampleFormat::I32 => build::<i32>(&device, &config, engine, Arc::clone(&failed)),
        SampleFormat::U8 => build::<u8>(&device, &config, engine, Arc::clone(&failed)),
        SampleFormat::U16 => build::<u16>(&device, &config, engine, Arc::clone(&failed)),
        SampleFormat::U32 => build::<u32>(&device, &config, engine, Arc::clone(&failed)),
        other => return Err(format!("the sample format {other} is not supported")),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(Box::new(DeviceOutput {
        stream: Some(stream),
        failed,
    }))
}

/// Build a stream that converts the mono samples of the engine to the
/// sample format `T`, the same sample on each channel of a frame. A stream
/// error sets `failed`.
fn build<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    engine: SharedEngine,
    failed: Arc<AtomicBool>,
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
            // No text here: the guard of `GuardedOutput` changes to the
            // clock output and writes the note (issue 212).
            move |_error| failed.store(true, Ordering::SeqCst),
            None,
        )
        .map_err(|e| e.to_string())
}
