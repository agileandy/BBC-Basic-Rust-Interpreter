//! The map from a SOUND pitch value (0 to 255) to the 10-bit divider of the
//! SN76489, and from a divider to a frequency.
//!
//! Source: the `setPitch` routine of MOS 1.20 at &ED16 and its pitch tables at
//! &EDFB and &EE07, in the annotated disassembly of TobyLobster
//! (<https://tobylobster.github.io/mos/mos/S-s16.html>, chapter 16, sections
//! 14 to 17). The channel offsets are at `soundPitchOffsetByChannelTable`
//! (chapter 4 of the same disassembly).
//!
//! The rule of the MOS:
//! 1. Bits 0 and 1 of the pitch are the quarter steps (0 to 3) above a semitone.
//! 2. The pitch divided by 4 is a count of semitones. This count divided by 12
//!    is the octave (0 to 5), and the remainder is the note (B, C, C# ... A#).
//! 3. The table gives a 10-bit divider for the note, and a step to subtract for
//!    each quarter step.
//! 4. The divider is halved once for each octave (the fraction is lost).
//! 5. The offset of the channel (0, 0, 1 or 2) is added.
//!
//! The BBC Micro User Guide (chapter 30, Sound, and the SOUND keyword entry)
//! names pitch 53 as middle C and pitch 89 as the A of 440 Hz. The MOS table
//! starts at B for pitch 0, so these values are approximately a quarter of a
//! semitone sharp on a real machine: pitch 53 is 266.5 Hz and pitch 89 is
//! 449.6 Hz. This module follows the MOS, because the MOS drives the chip.

/// The clock of the SN76489 on the BBC Micro, in Hz.
pub const CHIP_CLOCK_HZ: u32 = 4_000_000;

/// Bits 0 to 7 of the 10-bit divider of each note (B, C, C# ... A#).
const DIVIDER_LOW: [u8; 12] = [
    0xF0, 0xB7, 0x82, 0x4F, 0x20, 0xF3, 0xC8, 0xA0, 0x7B, 0x57, 0x35, 0x16,
];

/// Bits 0 and 1: bits 8 and 9 of the divider. Bits 4 to 7: the step to
/// subtract for each quarter step above the note.
const DIVIDER_HIGH: [u8; 12] = [
    0xE7, 0xD7, 0xCB, 0xC3, 0xB7, 0xAA, 0xA2, 0x9A, 0x92, 0x8A, 0x82, 0x7A,
];

/// The MOS adds this offset to the divider of SOUND channel 0, 1, 2 and 3.
const CHANNEL_OFFSET: [u16; 4] = [0, 0, 1, 2];

/// Gives the 10-bit divider that MOS 1.20 sends to the chip for a SOUND
/// `pitch` (0 to 255) on a SOUND `channel` (0 to 3).
///
/// For channel 0 (noise), the MOS does not use this map. The value is the
/// divider that the channel offset 0 gives.
///
/// # Panics
///
/// Panics if `channel` is more than 3.
pub fn divider(channel: u8, pitch: u8) -> u16 {
    assert!(channel <= 3, "channel must be 0 to 3, not {channel}");
    let quarter = u16::from(pitch & 3);
    let semitones = pitch >> 2;
    let octave = semitones / 12;
    let note = usize::from(semitones % 12);
    let base = (u16::from(DIVIDER_HIGH[note] & 3) << 8) | u16::from(DIVIDER_LOW[note]);
    let step = u16::from(DIVIDER_HIGH[note] >> 4);
    let value = (base - quarter * step) >> octave;
    (value + CHANNEL_OFFSET[usize::from(channel)]) & 0x3FF
}

/// Gives the frequency in Hz of the square wave of a tone channel with a
/// 10-bit `divider`: 4 MHz / (32 x divider). The chip uses a divider of 0
/// as 1024.
pub fn frequency(divider: u16) -> f64 {
    f64::from(CHIP_CLOCK_HZ) / (32.0 * f64::from(effective_divider(divider)))
}

/// The count that the chip uses for a 10-bit divider: 0 is 1024.
pub(crate) fn effective_divider(divider: u16) -> u16 {
    match divider & 0x3FF {
        0 => 1024,
        d => d,
    }
}
