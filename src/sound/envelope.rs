//! The ENVELOPE statement: the shape of the pitch and the amplitude of a
//! note (issue #209).
//!
//! `ENVELOPE N, T, PI1, PI2, PI3, PN1, PN2, PN3, AA, AD, AS, AR, ALA, ALD`
//! (BBC Micro User Guide, the ENVELOPE keyword page and chapter 30, Sound).
//! BASIC 2 evaluates the 14 values and gives their low bytes to OSWORD 8
//! (BASIC 2 ROM at &B472). The rules here are those of MOS 1.20, from the
//! annotated disassembly of TobyLobster:
//!
//! - OSWORD 8 (chapter 15, section 36) keeps each envelope in 16 bytes.
//! - `processSoundInterrupt` (chapter 16, section 6) does one step of the
//!   envelope of each channel at each T ticks of 1/100 second.
//! - `readNextSoundFromBufferAndProcess` (chapter 16, section 15) starts
//!   the envelope of a note.
//!
//! <https://tobylobster.github.io/mos/mos/S-s15.html> and
//! <https://tobylobster.github.io/mos/mos/S-s16.html>
//!
//! The state machine here has no clock and no chip, so that a test can check
//! the level and the pitch after each step.

/// The count of envelope places. Envelope N goes in place (N - 1) AND 15,
/// as OSWORD 8 calculates it. On a Model B, envelopes 1 to 4 are in the
/// envelope buffer at &08C0. Envelopes 5 to 16 go in the tape and RS423
/// output buffer at &0900, which a program with no speech, tape or RS423
/// output can use. This model keeps all 16 places.
pub const ENVELOPES: usize = 16;

/// The MOS volume of a silent channel ($C0). The MOS volume goes from -64
/// (silent) to 63 ($3F, the loudest).
pub const SILENT: i8 = -64;

/// The ADSR phase at the end of a note (MOS: release in progress).
pub(super) const PHASE_RELEASE: u8 = 3;
/// The ADSR phase after the sound ended (MOS: release complete).
pub(super) const PHASE_ENDED: u8 = 4;

/// The number of pitch sections.
const SECTIONS: u8 = 3;

/// One envelope: the 16 bytes of OSWORD 8. Byte 0 is T, bytes 1 to 3 are
/// PI1 to PI3, bytes 4 to 6 are PN1 to PN3, bytes 7 to 10 are AA, AD, AS and
/// AR, bytes 11 and 12 are ALA and ALD. Bytes 13 to 15 are 0, so the target
/// of the sustain and the release phases is silence.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Envelope([u8; 16]);

impl Envelope {
    /// The envelope of the 13 values after N, in the order of the ENVELOPE
    /// statement. Each value uses its low byte, as BASIC 2 does.
    pub fn new(values: [i32; 13]) -> Self {
        let mut bytes = [0u8; 16];
        for (byte, value) in bytes.iter_mut().zip(values) {
            *byte = value as u8;
        }
        Self(bytes)
    }

    /// The ticks of 1/100 second in each step (bits 0 to 6 of T). The MOS
    /// does a step each tick if this is 0 or 1.
    pub fn step_ticks(&self) -> u8 {
        self.0[0] & 0x7F
    }

    /// Bit 7 of T is clear: the pitch sections start again after the last.
    fn repeats(&self) -> bool {
        self.0[0] & 0x80 == 0
    }
}

/// The place of envelope `number` in the 16 places (OSWORD 8).
pub fn envelope_index(number: i32) -> usize {
    usize::from((number as u8).wrapping_sub(1) & 15)
}

/// The chip level (0 silent, 15 the loudest) of a MOS volume, as
/// `setChannelXVolume` calculates it (chapter 16, section 3). An envelope
/// level of 0 to 126 is a MOS volume of -63 to 63, and thus a chip level of
/// (level + 1) DIV 8.
pub fn chip_level(volume: i8) -> u8 {
    ((volume as u8).wrapping_sub(0x40) >> 3) & 15
}

/// The MOS volume of a SOUND amplitude of -15 to 0, from byte 0 of the note
/// in the queue (bit 7 set, bits 3 to 6 = 15 + amplitude), as
/// `readNextSoundFromBufferAndProcess` calculates it.
pub fn note_volume(first: u8) -> i8 {
    // AND #$F8, ASL, EOR #$FF, LSR, SEC, SBC #$40
    ((((first << 1) & 0xF0) ^ 0xFF) >> 1).wrapping_sub(0x40) as i8
}

/// The envelope state of one channel: the part of the MOS sound workspace
/// that the steps change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shape {
    /// The MOS volume, -64 to 63
    volume: i8,
    /// The ADSR phase: 0 attack, 1 decay, 2 sustain, 3 release, 4 ended
    phase: u8,
    /// The pitch at the start of the note
    base_pitch: u8,
    /// The pitch section, 0 to 2, 3 after the last section, $FF at the start
    section: u8,
    /// The steps left in the pitch section
    section_left: u8,
    /// The pitch now is the base pitch + this offset
    pitch_offset: u8,
}

impl Default for Shape {
    /// A silent channel after a reset (MOS `clearSoundChannels`)
    fn default() -> Self {
        Self {
            volume: SILENT,
            phase: PHASE_ENDED,
            base_pitch: 0,
            section: SECTIONS,
            section_left: 0,
            pitch_offset: 0,
        }
    }
}

impl Shape {
    /// Start the envelope of a note at `pitch`. The volume does not change:
    /// the attack starts from the volume that the channel has.
    pub fn start(&mut self, pitch: u8) {
        self.phase = 0;
        self.base_pitch = pitch;
        self.section = 0xFF;
        self.section_left = 0;
        self.pitch_offset = 0;
    }

    /// The duration of the note ended: go to the release phase (MOS
    /// `checkForNextSoundToPlay`).
    pub fn release(&mut self) {
        if self.phase != PHASE_ENDED {
            self.phase = PHASE_RELEASE;
        }
    }

    /// Set the volume of a note with no envelope.
    pub fn set_volume(&mut self, volume: i8) {
        self.volume = volume;
    }

    /// Silence the channel (MOS `silenceChannelX`).
    pub fn silence(&mut self) {
        self.phase = PHASE_ENDED;
        self.volume = SILENT;
    }

    /// True after the release phase.
    pub fn ended(&self) -> bool {
        self.phase == PHASE_ENDED
    }

    /// The MOS volume, -64 to 63.
    #[cfg(test)]
    pub fn volume(&self) -> i8 {
        self.volume
    }

    /// The chip level, 0 to 15.
    pub fn level(&self) -> u8 {
        chip_level(self.volume)
    }

    /// The pitch: the base pitch and the changes of the pitch sections.
    pub fn pitch(&self) -> u8 {
        self.base_pitch.wrapping_add(self.pitch_offset)
    }

    /// One step of `envelope`: the amplitude phase, then the pitch section.
    /// Give the new pitch if the pitch section changed it.
    pub fn step(&mut self, envelope: &Envelope) -> Option<u8> {
        if self.phase != PHASE_ENDED {
            self.step_amplitude(envelope);
        }
        self.step_pitch(envelope)
    }

    /// MOS 1.20 chapter 16, section 6, from "set target amplitude for new
    /// phase" to `adjustEnvelopePitch`.
    fn step_amplitude(&mut self, envelope: &Envelope) {
        let phase = usize::from(self.phase);
        let target = envelope.0[11 + phase].wrapping_sub(0x3F) as i8;
        let change = envelope.0[7 + phase] as i8;
        // An overflow gives the loudest ($3F) or silence ($C0), and the
        // volume stays in -64 to 63
        let volume =
            self.volume
                .checked_add(change)
                .unwrap_or(if change > 0 { 63 } else { SILENT });
        self.volume = volume.clamp(SILENT, 63);
        // The volume reached the target if the sign of (volume - target) is
        // that of (change - 1). A change of 0 thus jumps to a higher target.
        if (self.volume.wrapping_sub(target) ^ change.wrapping_sub(1)) >= 0 {
            self.volume = target;
            self.phase += 1;
        }
    }

    /// MOS 1.20 chapter 16, section 6, from `adjustEnvelopePitch` to
    /// `skipToNextChannel`.
    fn step_pitch(&mut self, envelope: &Envelope) -> Option<u8> {
        if self.section == SECTIONS {
            return None;
        }
        if self.section_left == 0 {
            self.section = self.section.wrapping_add(1);
            if self.section == SECTIONS {
                if !envelope.repeats() {
                    return None;
                }
                self.pitch_offset = 0;
                self.section = 0;
            }
            self.section_left = envelope.0[4 + usize::from(self.section)];
            if self.section_left == 0 {
                return None;
            }
        }
        self.section_left -= 1;
        let change = envelope.0[1 + usize::from(self.section)];
        self.pitch_offset = self.pitch_offset.wrapping_add(change);
        Some(self.pitch())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ENVELOPE values after N: T, PI1, PI2, PI3, PN1, PN2, PN3, AA, AD,
    /// AS, AR, ALA, ALD.
    fn envelope(values: [i32; 13]) -> Envelope {
        Envelope::new(values)
    }

    /// The (level, pitch) after each of `count` steps of a note at `pitch`
    /// that starts on a silent channel.
    fn steps(envelope: &Envelope, pitch: u8, count: usize) -> Vec<(u8, u8)> {
        let mut shape = Shape::default();
        shape.start(pitch);
        (0..count)
            .map(|_| {
                shape.step(envelope);
                (shape.level(), shape.pitch())
            })
            .collect()
    }

    fn pitches(envelope: &Envelope, pitch: u8, count: usize) -> Vec<u8> {
        steps(envelope, pitch, count)
            .iter()
            .map(|(_, pitch)| *pitch)
            .collect()
    }

    fn levels(envelope: &Envelope, count: usize) -> Vec<u8> {
        steps(envelope, 100, count)
            .iter()
            .map(|(level, _)| *level)
            .collect()
    }

    #[test]
    fn a_pitch_sweep_up_adds_the_change_at_each_step() {
        let sweep = envelope([1, 4, 0, 0, 10, 0, 0, 0, 0, 0, 0, 126, 126]);

        let found = pitches(&sweep, 100, 10);

        assert_eq!(found[0], 104, "after 1 step");
        assert_eq!(found[1], 108, "after 2 steps");
        assert_eq!(found[9], 140, "after 10 steps");
    }

    #[test]
    fn the_3_pitch_sections_follow_each_other() {
        let sections = envelope([129, 2, -3, 1, 2, 2, 1, 0, 0, 0, 0, 126, 126]);

        let found = pitches(&sections, 50, 8);

        // Each section starts in the step after the last step of the section
        // before it, and after the 3 sections the pitch does not change
        assert_eq!(found, vec![52, 54, 51, 48, 49, 49, 49, 49]);
    }

    #[test]
    fn the_auto_repeat_starts_the_pitch_sections_again_from_the_base_pitch() {
        let repeat = envelope([1, 4, 0, 0, 2, 0, 0, 0, 0, 0, 0, 126, 126]);
        let once = envelope([0x81, 4, 0, 0, 2, 0, 0, 0, 0, 0, 0, 126, 126]);

        // MOS 1.20: a section with no steps takes one step with no change.
        // Sections 2 and 3 have no steps, so a cycle has 4 steps.
        assert_eq!(
            pitches(&repeat, 100, 9),
            vec![104, 108, 108, 108, 104, 108, 108, 108, 104]
        );
        assert_eq!(
            pitches(&once, 100, 9),
            vec![104, 108, 108, 108, 108, 108, 108, 108, 108]
        );
    }

    #[test]
    fn the_attack_and_the_decay_go_to_the_2_target_levels() {
        // AA = 32 to ALA = 126, then AD = -8 to ALD = 63, then a sustain
        // with no change
        let shape = envelope([1, 0, 0, 0, 0, 0, 0, 32, -8, 0, -16, 126, 63]);

        let found = levels(&shape, 16);

        // Attack: -64 + 32 at each step, -32, 0, 32, then 63 (the target).
        // Decay: 55, 47, ..., 7, then 0 (the target, level 8). Sustain: 0.
        assert_eq!(
            found,
            vec![4, 8, 12, 15, 14, 13, 12, 11, 10, 9, 8, 8, 8, 8, 8, 8]
        );
    }

    #[test]
    fn the_release_goes_to_silence_and_ends() {
        let shape = envelope([1, 0, 0, 0, 0, 0, 0, 127, 0, 0, -16, 126, 126]);
        let mut state = Shape::default();
        state.start(100);
        state.step(&shape);
        assert_eq!(state.level(), 15, "the attack reaches 126 in one step");

        state.release();
        let mut found = Vec::new();
        for _ in 0..6 {
            state.step(&shape);
            found.push((state.volume(), state.ended()));
        }

        assert_eq!(
            found,
            vec![
                (47, false),
                (31, false),
                (15, false),
                (-1, false),
                (-17, false),
                (-33, false)
            ]
        );
        for _ in 0..3 {
            state.step(&shape);
        }
        assert_eq!(
            (state.volume(), state.level(), state.ended()),
            (-63, 0, true)
        );
    }

    #[test]
    fn a_step_of_amplitude_stops_at_the_loudest_and_at_silence() {
        // ALA = 190 and 191 give targets out of the range (127 and -128), so
        // the attack does not reach its target
        let up = envelope([1, 0, 0, 0, 0, 0, 0, 127, 0, 0, 0, 190, 0]);
        let down = envelope([1, 0, 0, 0, 0, 0, 0, -127, 0, 0, 0, 191, 0]);
        let mut state = Shape::default();
        state.start(0);
        state.set_volume(40);

        state.step(&up);
        // 40 + 127 overflows, so the MOS uses $3F
        assert_eq!(state.volume(), 63);
        state.step(&up);
        assert_eq!(state.volume(), 63);

        state.set_volume(-40);
        state.step(&down);
        // -40 - 127 overflows, so the MOS uses $C0
        assert_eq!(state.volume(), SILENT);
    }

    #[test]
    fn an_envelope_with_no_values_is_silent() {
        assert_eq!(levels(&Envelope::default(), 6), vec![0; 6]);
    }

    #[test]
    fn the_step_length_and_the_place_of_an_envelope() {
        assert_eq!(envelope([0x85; 13]).step_ticks(), 5);
        assert_eq!(envelope([300; 13]), envelope([44; 13]), "low bytes");
        assert_eq!(
            [1, 4, 5, 16, 17, 0].map(envelope_index),
            [0, 3, 4, 15, 0, 15]
        );
    }

    #[test]
    fn the_chip_level_of_a_mos_volume() {
        // MOS 1.20 chapter 16, section 3: $3F is the loudest, $07 is the
        // middle level and $C7 to $C0 are silent
        assert_eq!(
            [63, 7, -57, -64, 0, -56].map(chip_level),
            [15, 8, 0, 0, 8, 1]
        );
        // SOUND amplitudes -15, -8 and 0 in byte 0 of a queued note
        assert_eq!([0x80, 0xB8, 0xF8].map(note_volume), [63, 7, -57]);
    }
}
