//! The sound engine: the queues of the SOUND statement and their clock, in
//! front of the SN76489 model.
//!
//! The clock is the count of samples that the output took with `fill`, not
//! the wall clock. A recording to a file and a test thus get the same timing
//! as the speakers.

//!
//! The rules are those of MOS 1.20, which the SOUND statement of BBC BASIC 2
//! calls with OSWORD 7 and no checks of its own: `osword7EntryPoint` (chapter
//! 15, section 31) puts a note in a queue, and `processSoundInterrupt`
//! (chapter 16, sections 6 to 15) plays the queues 100 times each second.
//! Source: the annotated disassembly of TobyLobster,
//! <https://tobylobster.github.io/mos/mos/S-s16.html>. The BBC Micro User
//! Guide describes the same rules on the SOUND keyword page and in chapter
//! 30, Sound.

use super::chip::{Chip, DEFAULT_SAMPLE_RATE};
use super::pitch::divider;
use std::collections::VecDeque;

/// The ticks of the sound interrupt each second.
const TICKS_PER_SECOND: u64 = 100;

/// A queue has 16 bytes, and a full queue has one byte free. A note takes
/// 3 bytes, so a queue holds 5 notes (MOS 1.20 buffer table, chapter 2).
const QUEUE_BYTES: u8 = 15;
const NOTE_BYTES: u8 = 3;
const QUEUE_NOTES: usize = (QUEUE_BYTES / NOTE_BYTES) as usize;

/// The ticks in each step of a duration: a duration is in 1/20 second.
const TICKS_PER_STEP: u8 = 5;

/// A duration of 255 does not end. Only a flush stops it.
const ENDLESS: u8 = 255;

/// The level of a note with an envelope (amplitude 1 to 4). Issue #209 gives
/// the envelopes. Until then such a note plays at this fixed middle level.
const ENVELOPE_LEVEL: u8 = 8;

/// The phase of a channel after its sound ended (MOS: release complete).
const PHASE_ENDED: u8 = 4;
/// The phase of a channel at the end of a note (MOS: release in progress).
const PHASE_RELEASE: u8 = 3;

/// No sync is pending (MOS `soundSyncCount` = $FF).
const NO_SYNC: u8 = 0xFF;

/// A note that started to play, for a recording or a test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartedNote {
    /// The channel, 0 to 3
    pub channel: u8,
    /// The chip level, 0 (silence) to 15 (the loudest)
    pub level: u8,
    /// The pitch, 0 to 255
    pub pitch: u8,
    /// The duration in 1/20 second, 255 for no end
    pub duration: u8,
    /// The sample of the engine clock at which the note started
    pub start: u64,
}

/// A note in a queue: the 3 bytes of MOS 1.20. Byte 0 has the level or the
/// envelope (bits 3 to 7), the hold bit (bit 2) and the sync count (bits 0
/// and 1).
type QueuedNote = [u8; 3];

/// The state of one channel, as in the MOS sound workspace.
#[derive(Debug, Clone)]
struct Channel {
    queue: VecDeque<QueuedNote>,
    /// A note was queued, and the channel did not end (MOS occupancy)
    active: bool,
    /// A flush emptied the queue (bit 7 of the MOS buffer empty flag)
    flushed: bool,
    /// The steps of 1/20 second left
    duration: u8,
    /// The ticks left in this step
    countdown: u8,
    /// The channel waits for the other channels of a sync
    sync_waiting: bool,
    phase: u8,
    level: u8,
    /// The last pitch that went to the chip
    pitch: u8,
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            queue: VecDeque::with_capacity(QUEUE_NOTES),
            active: false,
            flushed: false,
            duration: 0,
            // The value at the end of a note. A hold note on a new channel
            // thus has the same length as after a note.
            countdown: TICKS_PER_STEP,
            sync_waiting: false,
            phase: PHASE_ENDED,
            level: 0,
            pitch: 0,
        }
    }
}

/// The engine owns the chip and the queues. An output takes its samples
/// with `fill`.
#[derive(Debug, Clone)]
pub struct SoundEngine {
    chip: Chip,
    channels: [Channel; 4],
    /// The other channels that a sync still needs (MOS `soundSyncCount`)
    sync_count: u8,
    /// The count of samples that `fill` gave
    clock: u64,
    /// The ticks done since `tick_base`
    ticks: u64,
    /// The clock at the last change of the sample rate
    tick_base: u64,
    /// The notes that started, if a caller asked for a record
    record: Option<Vec<StartedNote>>,
    /// A SOUND statement waits for a place in a queue
    waiting: bool,
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
            channels: Default::default(),
            sync_count: NO_SYNC,
            clock: 0,
            ticks: 0,
            tick_base: 0,
            record: None,
            waiting: false,
        }
    }

    /// The samples each second.
    pub fn sample_rate(&self) -> u32 {
        self.chip.sample_rate()
    }

    /// Use the sample rate of the output. The output calls this before its
    /// first `fill`. The notes that play continue.
    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        self.chip = Chip::new(sample_rate);
        self.tick_base = self.clock;
        self.ticks = 0;
        for channel in 0..4u8 {
            let state = &self.channels[usize::from(channel)];
            let (level, pitch) = (state.level, state.pitch);
            self.send_pitch(channel, pitch);
            self.chip.set_level(channel, level);
        }
    }

    /// The count of samples that `fill` gave since the engine started.
    pub fn clock(&self) -> u64 {
        self.clock
    }

    /// Fill `buffer` with the next samples (-1.0 to 1.0) and advance the
    /// clock by its length. The call does not wait.
    pub fn fill(&mut self, buffer: &mut [f32]) {
        let mut done = 0;
        while done < buffer.len() {
            let next_tick =
                self.tick_base + self.ticks * u64::from(self.sample_rate()) / TICKS_PER_SECOND;
            if self.clock >= next_tick {
                self.tick();
                self.ticks += 1;
                continue;
            }
            let count = ((next_tick - self.clock) as usize).min(buffer.len() - done);
            self.chip.render(&mut buffer[done..done + count]);
            done += count;
            self.clock += count as u64;
        }
    }

    /// Put a note in a queue: the 4 values of `SOUND channel, amplitude,
    /// pitch, duration`. Each value uses its low 16 bits, as OSWORD 7 does.
    ///
    /// The channel is &HSFC: C (0 to 3) is the channel, F (not 0) flushes the
    /// queue and stops the note that plays, S (0 to 3) is the count of other
    /// channels to start with, and H (not 0) holds the last sound for the
    /// duration. A high byte of &20 or more is not a sound: nothing happens.
    ///
    /// Give false if the queue is full. The note is then not queued, and the
    /// caller tries again.
    pub fn try_sound(&mut self, channel: i32, amplitude: i32, pitch: i32, duration: i32) -> bool {
        let [low, high] = (channel as u16).to_le_bytes();
        if high >= 0x20 {
            return true;
        }
        let number = low & 3;
        if low >= 0x10 {
            self.flush_channel(number);
        }
        let state = &mut self.channels[usize::from(number)];
        if state.queue.len() >= QUEUE_NOTES {
            return false;
        }
        let hold = u8::from(high >= 0x10);
        // The encoding of OSWORD 7: ((amplitude * 2 + hold) - 2) * 4 + sync
        let first = ((amplitude as u8) << 1 | hold).wrapping_sub(2) << 2 | (high & 3);
        state.queue.push_back([first, pitch as u8, duration as u8]);
        state.active = true;
        true
    }

    /// The free bytes in the queue of `channel` (0 to 3): ADVAL(-5 - channel).
    /// An empty queue has 15, and each note takes 3.
    pub fn free_space(&self, channel: u8) -> u8 {
        let notes = self.channels[usize::from(channel & 3)].queue.len() as u8;
        QUEUE_BYTES - notes * NOTE_BYTES
    }

    /// Empty all queues and silence all channels, as the MOS does when it
    /// acknowledges an Escape.
    pub fn flush(&mut self) {
        for channel in 0..4 {
            self.flush_channel(channel);
        }
    }

    /// A SOUND statement waits for a place in a queue (true) or not. An
    /// output in a test can advance the clock only while a statement waits.
    pub fn set_waiting(&mut self, waiting: bool) {
        self.waiting = waiting;
    }

    /// True while a SOUND statement waits for a place in a queue.
    pub fn is_waiting(&self) -> bool {
        self.waiting
    }

    /// Record each note that starts, for `take_notes`.
    pub fn record_notes(&mut self) {
        self.record.get_or_insert_with(Vec::new);
    }

    /// Give the notes that started since the last call, if `record_notes`
    /// started a record.
    pub fn take_notes(&mut self) -> Vec<StartedNote> {
        self.record.as_mut().map(std::mem::take).unwrap_or_default()
    }

    /// MOS `clearSoundChannelBuffer` and `flushSoundBufferX`.
    fn flush_channel(&mut self, channel: u8) {
        self.silence(channel);
        self.send_pitch(channel, 0);
        let state = &mut self.channels[usize::from(channel)];
        state.queue.clear();
        state.duration = 0;
        state.active = false;
        state.flushed = true;
        for other in &mut self.channels {
            other.sync_waiting = false;
        }
        self.sync_count = NO_SYNC;
    }

    /// One tick of the sound interrupt (MOS `processSoundInterrupt`). The
    /// channels go from 3 to 0, as in the MOS.
    fn tick(&mut self) {
        let mut release_sync = false;
        if self.sync_count == 0 {
            release_sync = true;
            self.sync_count = NO_SYNC;
        }
        for channel in (0..4u8).rev() {
            let state = &self.channels[usize::from(channel)];
            if !state.active {
                continue;
            }
            if state.flushed || state.duration == 0 {
                self.next_sound(channel, release_sync);
            }
            let state = &mut self.channels[usize::from(channel)];
            match state.duration {
                0 => self.next_sound(channel, release_sync),
                ENDLESS => {}
                _ => {
                    state.countdown = state.countdown.wrapping_sub(1);
                    if state.countdown == 0 {
                        state.countdown = TICKS_PER_STEP;
                        state.duration -= 1;
                        if state.duration == 0 {
                            self.next_sound(channel, release_sync);
                        }
                    }
                }
            }
        }
    }

    /// MOS `checkForNextSoundToPlay` and `syncSounds`: the sound of a
    /// channel ended, so start the next note if it can start.
    fn next_sound(&mut self, channel: u8, release_sync: bool) {
        let state = &mut self.channels[usize::from(channel)];
        if state.phase != PHASE_ENDED {
            state.phase = PHASE_RELEASE;
        }
        if state.flushed {
            state.flushed = false;
            state.duration = 0;
            for other in &mut self.channels {
                other.sync_waiting = false;
            }
            self.sync_count = NO_SYNC;
        }
        let state = &mut self.channels[usize::from(channel)];
        if state.sync_waiting {
            if release_sync {
                state.sync_waiting = false;
                self.read_note(channel);
            } else {
                self.silence(channel);
            }
            return;
        }
        let Some(&[first, ..]) = state.queue.front() else {
            // MOS `syncSoundBufferEmpty`
            if state.phase == PHASE_ENDED {
                state.active = false;
            }
            self.silence(channel);
            return;
        };
        let sync = first & 3;
        if sync == 0 {
            self.read_note(channel);
            return;
        }
        if self.sync_count != 0 {
            state.sync_waiting = true;
            self.sync_count = if self.sync_count == NO_SYNC {
                sync
            } else {
                self.sync_count - 1
            };
        }
        self.silence(channel);
    }

    /// MOS `readNextSoundFromBufferAndProcess`: take the next note out of the
    /// queue and play it.
    fn read_note(&mut self, channel: u8) {
        let state = &mut self.channels[usize::from(channel)];
        let Some([first, pitch, duration]) = state.queue.pop_front() else {
            return;
        };
        if first & 4 != 0 {
            // A hold note: with no envelope, a silence for its duration
            self.silence(channel);
            self.channels[usize::from(channel)].duration = duration;
            return;
        }
        let level = if first & 0x80 != 0 {
            15 - ((first >> 3) & 15)
        } else {
            ENVELOPE_LEVEL
        };
        let state = &mut self.channels[usize::from(channel)];
        state.countdown = TICKS_PER_STEP;
        state.phase = 0;
        state.duration = duration;
        self.set_level(channel, level);
        if pitch != self.channels[usize::from(channel)].pitch {
            self.send_pitch(channel, pitch);
        }
        let start = self.clock;
        if let Some(record) = self.record.as_mut() {
            record.push(StartedNote {
                channel,
                level,
                pitch,
                duration,
                start,
            });
        }
    }

    /// MOS `silenceChannelX`
    fn silence(&mut self, channel: u8) {
        self.channels[usize::from(channel)].phase = PHASE_ENDED;
        self.set_level(channel, 0);
    }

    fn set_level(&mut self, channel: u8, level: u8) {
        self.channels[usize::from(channel)].level = level;
        self.chip.set_level(channel, level);
    }

    /// MOS `setPitch`: send a pitch to the chip. The noise channel uses the
    /// low 3 bits as the noise setting.
    fn send_pitch(&mut self, channel: u8, pitch: u8) {
        let state = &mut self.channels[usize::from(channel)];
        state.pitch = pitch;
        let level = state.level;
        if channel == 0 {
            self.chip.set_noise(pitch & 7, level);
        } else {
            self.chip.set_tone(channel, divider(channel, pitch), level);
        }
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

    /// The samples of one tick (1/100 second) at 44100 samples each second.
    const TICK: u64 = 441;

    /// An engine that records its notes.
    fn recording() -> SoundEngine {
        let mut engine = SoundEngine::default();
        engine.record_notes();
        engine
    }

    /// Advance the engine by `ticks` ticks, and give the samples.
    fn run(engine: &mut SoundEngine, ticks: u64) -> Vec<f32> {
        let mut buffer = vec![0.0f32; (ticks * TICK) as usize];
        engine.fill(&mut buffer);
        buffer
    }

    /// The (channel, start tick) of each note that started.
    fn starts(engine: &mut SoundEngine) -> Vec<(u8, u64)> {
        engine
            .take_notes()
            .iter()
            .map(|note| (note.channel, note.start / TICK))
            .collect()
    }

    #[test]
    fn three_notes_on_one_channel_play_one_after_the_other() {
        let mut engine = recording();
        for pitch in [53, 69, 89] {
            assert!(engine.try_sound(1, -15, pitch, 2));
        }

        run(&mut engine, 40);

        // MOS 1.20: the first note takes 5 ticks for each step, from the tick
        // that reads it, so it ends one tick early (at tick 9). A note that
        // the end of the last note reads has 10 ticks.
        let notes = engine.take_notes();
        let found: Vec<(u8, u8, u64)> = notes
            .iter()
            .map(|note| (note.pitch, note.level, note.start / TICK))
            .collect();
        assert_eq!(found, vec![(53, 15, 0), (69, 15, 9), (89, 15, 19)]);
        assert!(notes.iter().all(|note| note.start % TICK == 0));
    }

    #[test]
    fn a_queue_has_room_for_5_notes_and_a_sixth_waits() {
        let mut engine = recording();
        assert_eq!(engine.free_space(1), 15);
        for _ in 0..5 {
            assert!(engine.try_sound(1, -10, 100, 10));
        }
        assert_eq!(engine.free_space(1), 0);
        assert!(!engine.try_sound(1, -10, 100, 10), "a full queue refuses");

        run(&mut engine, 1);

        // The first tick reads the first note out of the queue
        assert_eq!(engine.free_space(1), 3);
        assert!(engine.try_sound(1, -10, 100, 10));
        assert_eq!(engine.free_space(2), 15, "other queues are empty");
    }

    #[test]
    fn the_flush_bit_empties_the_queue_and_stops_the_note() {
        let mut engine = recording();
        for _ in 0..3 {
            assert!(engine.try_sound(2, -15, 100, 20));
        }
        run(&mut engine, 5);

        assert!(engine.try_sound(0x12, -15, 150, 1));
        assert_eq!(engine.free_space(2), 12);
        run(&mut engine, 5);

        let notes = engine.take_notes();
        let found: Vec<(u8, u64)> = notes.iter().map(|n| (n.pitch, n.start / TICK)).collect();
        assert_eq!(found, vec![(100, 0), (150, 5)]);
    }

    #[test]
    fn the_sync_bits_start_two_channels_together() {
        let mut engine = recording();
        assert!(engine.try_sound(0x101, -15, 100, 5));
        run(&mut engine, 3);
        assert!(
            engine.take_notes().is_empty(),
            "the note waits for a partner"
        );

        assert!(engine.try_sound(0x102, -15, 120, 5));
        run(&mut engine, 3);

        // The second channel joins at tick 3, and both start at tick 4
        assert_eq!(starts(&mut engine), vec![(2, 4), (1, 4)]);
    }

    #[test]
    fn a_hold_note_is_a_silence_for_its_duration() {
        let mut engine = recording();
        assert!(engine.try_sound(0x1001, -15, 100, 2));
        assert!(engine.try_sound(1, -15, 120, 2));

        let samples = run(&mut engine, 9);
        assert!(samples.iter().all(|s| *s == 0.0));
        run(&mut engine, 2);

        assert_eq!(starts(&mut engine), vec![(1, 9)]);
    }

    #[test]
    fn a_duration_of_255_plays_until_a_flush() {
        let mut engine = recording();
        assert!(engine.try_sound(3, -15, 100, 255));
        assert!(engine.try_sound(3, -15, 120, 1));
        run(&mut engine, 2000);
        assert_eq!(starts(&mut engine), vec![(3, 0)]);

        assert!(engine.try_sound(0x13, -15, 140, 1));
        run(&mut engine, 1);
        assert_eq!(starts(&mut engine), vec![(3, 2000)]);
    }

    #[test]
    fn a_duration_of_0_ends_in_the_tick_that_starts_it() {
        let mut engine = recording();
        assert!(engine.try_sound(1, -15, 100, 0));
        assert!(engine.try_sound(1, -15, 120, 0));

        let first_tick = run(&mut engine, 1);
        let second_tick = run(&mut engine, 1);

        // MOS 1.20: the tick that reads the first note finds no duration and
        // reads the second note. The second note plays until the next tick
        // finds no duration: a short sound of 1/100 second.
        assert_eq!(starts(&mut engine), vec![(1, 0), (1, 0)]);
        assert!(first_tick.iter().any(|s| *s != 0.0));
        assert!(second_tick.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn the_amplitude_gives_the_level() {
        let mut engine = recording();
        assert!(engine.try_sound(1, -15, 100, 1));
        assert!(engine.try_sound(2, -8, 100, 1));
        assert!(engine.try_sound(3, 0, 100, 1));
        assert!(engine.try_sound(0, 1, 4, 1));
        run(&mut engine, 1);

        let levels: Vec<(u8, u8)> = engine
            .take_notes()
            .iter()
            .map(|note| (note.channel, note.level))
            .collect();
        // Envelope 1 has a fixed middle level until issue #209
        assert_eq!(levels, vec![(3, 0), (2, 8), (1, 15), (0, 8)]);
    }

    #[test]
    fn a_note_sounds_and_ends_with_silence() {
        let mut engine = recording();
        assert!(engine.try_sound(1, -15, 89, 1));

        let note = run(&mut engine, 4);
        let after = run(&mut engine, 2);

        assert!(note.iter().any(|s| *s != 0.0));
        assert!(after.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn flush_empties_all_queues_and_silences_all_channels() {
        let mut engine = recording();
        for channel in 0..4 {
            assert!(engine.try_sound(channel, -15, 89, 255));
            assert!(engine.try_sound(channel, -15, 89, 255));
        }
        run(&mut engine, 1);

        engine.flush();

        assert!(run(&mut engine, 2).iter().all(|s| *s == 0.0));
        assert!((0..4).all(|channel| engine.free_space(channel) == 15));
    }

    #[test]
    fn a_channel_with_a_high_byte_of_20_or_more_is_not_a_sound() {
        let mut engine = recording();
        assert!(engine.try_sound(0x2001, -15, 89, 1));
        run(&mut engine, 2);

        assert!(engine.take_notes().is_empty());
        assert_eq!(engine.free_space(1), 15);
    }
}
