//! The sound mapping tool (issue #211): the map from each SOUND pitch value
//! (0 to 255) to its frequency and its nearest musical note, the nearest
//! pitch values for a note or a frequency, `NOTE(name$)`, and an interactive
//! list that plays the notes.
//!
//! The frequencies come from the MOS 1.20 rule of `pitch.rs` (the `setPitch`
//! routine, TobyLobster disassembly chapter 16, sections 14 to 17), not from
//! a formula. The note names use equal temperament with A4 = 440 Hz, and C4
//! is middle C.
//!
//! The BBC Micro User Guide (the SOUND keyword page and chapter 30, Sound)
//! names pitch 53 as middle C and gives 4 values to each semitone, so that
//! 89 is A4. On a real machine these values are about a quarter of a
//! semitone sharp: pitch 53 is 266.5 Hz, not 261.6 Hz. The table shows the
//! User Guide name in a column of its own. `NOTE` and the search give the
//! value whose real frequency is nearest to the note: C4 is 52 and A4 is 88.

use super::pitch::{divider, frequency};
use std::io::{self, Write};

/// The tone channel of the map. Channels 2 and 3 add 1 and 2 to the
/// divider, a change of less than half a percent.
const MAP_CHANNEL: u8 = 1;

/// The note names of one octave, from C.
const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// The note number of A4 (440 Hz), as in MIDI: C4 is 60.
const A4_NUMBER: i32 = 69;
const A4_HZ: f64 = 440.0;

/// The pitch value that the User Guide names C4, and its note number.
const GUIDE_C4_PITCH: i32 = 53;
const C4_NUMBER: i32 = 60;

/// A note out of the range of SOUND: its nearest value is more than this
/// count of cents away.
const MAX_NOTE_ERROR_CENTS: f64 = 50.0;

/// One pitch value of the map.
#[derive(Debug, Clone, PartialEq)]
pub struct PitchRow {
    /// The SOUND pitch value, 0 to 255
    pub pitch: u8,
    /// The 10-bit divider that the MOS sends to the chip
    pub divider: u16,
    /// The frequency in Hz
    pub frequency: f64,
    /// The nearest note, with its octave, for example "C4"
    pub note: String,
    /// The error from the exact frequency of `note`, in cents
    pub cents: f64,
    /// The name that the User Guide gives the value, for example "C4" or
    /// "C4+1/4"
    pub guide_name: String,
    /// True if this value is the nearest value to `note`
    pub nearest: bool,
}

/// The name of a note number, for example 60 is "C4".
fn note_name(number: i32) -> String {
    let octave = number.div_euclid(12) - 1;
    format!("{}{}", NOTE_NAMES[number.rem_euclid(12) as usize], octave)
}

/// The exact frequency of a note number.
fn note_frequency(number: i32) -> f64 {
    A4_HZ * 2f64.powf(f64::from(number - A4_NUMBER) / 12.0)
}

/// The interval from `to` up to `from`, in cents.
fn cents(from: f64, to: f64) -> f64 {
    1200.0 * (from / to).log2()
}

/// The User Guide name of a pitch value: 53 is C4, with 4 values to each
/// semitone.
fn guide_name(pitch: u8) -> String {
    let offset = i32::from(pitch) - GUIDE_C4_PITCH;
    let name = note_name(C4_NUMBER + offset.div_euclid(4));
    match offset.rem_euclid(4) {
        0 => name,
        quarter => format!("{name}+{quarter}/4"),
    }
}

/// The map of each pitch value 0 to 255 on channel 1.
pub fn pitch_map() -> Vec<PitchRow> {
    let mut rows: Vec<PitchRow> = (0..=255u8)
        .map(|pitch| {
            let divider = divider(MAP_CHANNEL, pitch);
            let frequency = frequency(divider);
            let number = A4_NUMBER + cents(frequency, A4_HZ).div_euclid(100.0) as i32;
            // The nearest note is `number` or the note above it
            let number = if cents(frequency, note_frequency(number)) > 50.0 {
                number + 1
            } else {
                number
            };
            PitchRow {
                pitch,
                divider,
                frequency,
                note: note_name(number),
                cents: cents(frequency, note_frequency(number)),
                guide_name: guide_name(pitch),
                nearest: false,
            }
        })
        .collect();
    // The mark goes to the lowest of the values with the least error: in
    // the high octaves, some values have the same divider
    for i in 0..rows.len() {
        let error = rows[i].cents.abs();
        let best = rows.iter().enumerate().all(|(j, row)| {
            row.note != rows[i].note
                || row.cents.abs() > error
                || (row.cents.abs() == error && j >= i)
        });
        rows[i].nearest = best;
    }
    rows
}

/// Read a note name such as "C4", "F#3" or "Gb3" (the letter can be lower
/// case), and give its note number. C4 is 60.
fn parse_note(name: &str) -> Option<i32> {
    let mut chars = name.chars();
    let letter = chars.next()?.to_ascii_uppercase();
    let base = match letter {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let rest = chars.as_str();
    let (shift, octave) = match rest.chars().next()? {
        '#' => (1, &rest[1..]),
        'b' => (-1, &rest[1..]),
        _ => (0, rest),
    };
    if octave.is_empty() || !octave.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let octave: i32 = octave.parse().ok()?;
    Some((octave + 1) * 12 + base + shift)
}

/// The pitch values sorted by their distance from `target` Hz, the nearest
/// first, with their error in cents.
fn nearest_to(target: f64) -> Vec<(PitchRow, f64)> {
    let mut rows: Vec<(PitchRow, f64)> = pitch_map()
        .into_iter()
        .map(|row| {
            let error = cents(row.frequency, target);
            (row, error)
        })
        .collect();
    rows.sort_by(|a, b| a.1.abs().total_cmp(&b.1.abs()));
    rows
}

/// The pitch value for a note name, for `NOTE(name$)`: the value whose
/// frequency is nearest to the note. Give an error for a name that is not a
/// note, and for a note out of the range of SOUND.
pub fn note_pitch(name: &str) -> Result<u8, String> {
    let number = parse_note(name).ok_or_else(|| name.to_string())?;
    let (row, error) = nearest_to(note_frequency(number))
        .into_iter()
        .next()
        .expect("the map has rows");
    if error.abs() > MAX_NOTE_ERROR_CENTS {
        return Err(name.to_string());
    }
    Ok(row.pitch)
}

/// Print the map of all pitch values, with a mark at the value that is
/// nearest to each note.
pub fn print_table(out: &mut impl Write) -> io::Result<()> {
    writeln!(
        out,
        "SOUND pitch values on channel {MAP_CHANNEL} (the MOS 1.20 dividers, a 4 MHz chip clock)."
    )?;
    writeln!(
        out,
        "The User Guide names (53 is C4, 89 is A4) are about a quarter of a semitone sharp on a real BBC Micro, and NOTE() gives the real pitch."
    )?;
    writeln!(out, "* marks the value that is nearest to each note.")?;
    writeln!(out)?;
    writeln!(out, "Pitch  Divider  Frequency     Note  Cents  User Guide")?;
    for row in pitch_map() {
        let line = format!(
            "{:>5}  {:>7}  {:>9.2} Hz  {:<4} {:>+6.1}  {:<8}{}",
            row.pitch,
            row.divider,
            row.frequency,
            row.note,
            row.cents,
            row.guide_name,
            if row.nearest { " *" } else { "" }
        );
        writeln!(out, "{}", line.trim_end())?;
    }
    Ok(())
}

/// The count of values that the search prints.
const NEAREST_COUNT: usize = 3;

/// Print the pitch values nearest to `query`: a note name such as C4 or
/// F#3, or a frequency in Hz. Give an error text for a query that is not a
/// note and not a frequency.
pub fn print_nearest(query: &str, out: &mut impl Write) -> Result<(), String> {
    let (title, target) = match (query.parse::<f64>(), parse_note(query)) {
        (Ok(hz), _) if hz.is_finite() && hz > 0.0 => (String::new(), hz),
        (_, Some(number)) => (format!("{query} is "), note_frequency(number)),
        _ => {
            return Err(format!(
                "Bad note: {query}. Give a note such as C4 or F#3, or a frequency in Hz"
            ))
        }
    };
    let write = |out: &mut dyn Write| -> io::Result<()> {
        writeln!(
            out,
            "{title}{target:.2} Hz. The nearest pitch values on channel {MAP_CHANNEL}:"
        )?;
        writeln!(out, "Pitch  Frequency     Cents  Note  User Guide")?;
        for (row, error) in nearest_to(target).into_iter().take(NEAREST_COUNT) {
            writeln!(
                out,
                "{:>5}  {:>9.2} Hz  {:>+6.1}  {:<4}  {}",
                row.pitch, row.frequency, error, row.note, row.guide_name
            )?;
        }
        Ok(())
    };
    write(out).map_err(|error| error.to_string())
}

/// The bytes of the keys of the play list. Up and Down send Escape, `[`
/// and a letter.
const ESCAPE: u8 = 0x1B;
const UP: u8 = b'A';
const DOWN: u8 = b'B';

/// The SOUND of the play list: channel 1 with the flush bit, so that a new
/// note stops the last one, at the loudest level for 1 second.
const PLAY_CHANNEL: i32 = 0x11;
const PLAY_AMPLITUDE: i32 = -15;
const PLAY_DURATION: i32 = 20;

/// The interactive list of notes. Up and Down select a note, Enter plays it
/// on `engine`, and q, Escape or the end of input ends the list.
/// `read_key` gives the next byte of input.
pub fn play_list(
    mut read_key: impl FnMut() -> Option<u8>,
    out: &mut impl Write,
    engine: &super::SharedEngine,
) -> io::Result<()> {
    let notes: Vec<PitchRow> = pitch_map().into_iter().filter(|row| row.nearest).collect();
    let mut selected = notes
        .iter()
        .position(|row| row.note == "C4")
        .expect("the map has C4");
    writeln!(
        out,
        "Up and Down select a note, Enter plays it, q ends the list."
    )?;
    loop {
        let row = &notes[selected];
        write!(
            out,
            "\r{:<4} pitch {:>3}  {:>8.2} Hz  {:>+6.1} cents\x1b[K",
            row.note, row.pitch, row.frequency, row.cents
        )?;
        out.flush()?;
        match read_key() {
            Some(b'\r' | b'\n') => {
                let pitch = i32::from(row.pitch);
                super::lock(engine).try_sound(PLAY_CHANNEL, PLAY_AMPLITUDE, pitch, PLAY_DURATION);
            }
            Some(ESCAPE) => match (read_key(), read_key()) {
                (Some(b'['), Some(UP)) => selected = (selected + 1).min(notes.len() - 1),
                (Some(b'['), Some(DOWN)) => selected = selected.saturating_sub(1),
                (Some(b'['), Some(_)) => {}
                _ => break,
            },
            Some(b'q' | b'Q') | None => break,
            Some(_) => {}
        }
    }
    writeln!(out)
}

/// `--sound-map --play`: the play list on the terminal, with the keys of
/// GET. The sound goes to the output that `factory` makes, for example the
/// audio device of issue #210.
pub fn play_interactive(factory: super::OutputFactory) -> io::Result<()> {
    use super::{lock, SharedEngine, SoundEngine};
    use std::sync::{Arc, Mutex};

    let engine: SharedEngine = Arc::new(Mutex::new(SoundEngine::default()));
    let mut output = factory(Arc::clone(&engine));
    let result = play_list(
        crate::executor::key_input::read_key,
        &mut io::stdout(),
        &engine,
    );
    lock(&engine).flush();
    output.stop();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_map_has_256_rows_with_the_mos_frequencies() {
        let map = pitch_map();

        assert_eq!(map.len(), 256);
        assert!(map
            .iter()
            .enumerate()
            .all(|(i, row)| usize::from(row.pitch) == i));
        // Pitch 53: divider 469, 4 MHz / (32 x 469) = 266.52 Hz
        assert_eq!(map[53].divider, 469);
        assert!((map[53].frequency - 266.52).abs() < 0.01);
        assert_eq!(map[53].note, "C4");
        assert!((map[53].cents - 32.1).abs() < 0.1, "{}", map[53].cents);
    }

    #[test]
    fn the_nearest_value_to_a4_is_the_value_nearest_to_440_hz() {
        let map = pitch_map();
        let closest = map
            .iter()
            .min_by(|a, b| {
                (a.frequency - 440.0)
                    .abs()
                    .total_cmp(&(b.frequency - 440.0).abs())
            })
            .unwrap();

        let marked: Vec<u8> = map
            .iter()
            .filter(|row| row.nearest && row.note == "A4")
            .map(|row| row.pitch)
            .collect();
        assert_eq!(closest.pitch, 88);
        assert_eq!(marked, vec![88]);
    }

    #[test]
    fn each_note_has_one_marked_value() {
        let map = pitch_map();
        let mut names: Vec<&str> = map
            .iter()
            .filter(|row| row.nearest)
            .map(|row| row.note.as_str())
            .collect();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count, "no note has two marks");
        assert!(names.contains(&"C4") && names.contains(&"B2"));
    }

    #[test]
    fn the_user_guide_column_names_53_as_c4_and_89_as_a4() {
        let map = pitch_map();
        assert_eq!(map[53].guide_name, "C4");
        assert_eq!(map[89].guide_name, "A4");
        assert_eq!(map[54].guide_name, "C4+1/4");
        assert_eq!(map[52].guide_name, "B3+3/4");
        assert_eq!(map[0].guide_name, "A#2+3/4");
    }

    #[test]
    fn note_names_give_the_nearest_real_pitch() {
        assert_eq!(note_pitch("C4"), Ok(52));
        assert_eq!(note_pitch("A4"), Ok(88));
        assert_eq!(note_pitch("F#3"), Ok(28));
        assert_eq!(note_pitch("Gb3"), Ok(28));
        assert_eq!(note_pitch("c4"), Ok(52));
    }

    #[test]
    fn a_wrong_or_unplayable_note_name_is_an_error() {
        for name in ["H4", "C", "C#", "4", "", "C4x", "C99", "C1", "C9"] {
            assert!(note_pitch(name).is_err(), "{name:?}");
        }
    }

    #[test]
    fn the_search_for_a_note_gives_the_nearest_values_first() {
        let mut out = Vec::new();
        print_nearest("C4", &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        let lines: Vec<&str> = text.lines().collect();

        assert!(lines[0].starts_with("C4 is 261.63 Hz."), "{text}");
        assert!(lines[2].trim_start().starts_with("52 "), "{text}");
        assert!(lines[2].contains("+10.1"), "{text}");
        assert!(lines[3].trim_start().starts_with("51 "), "{text}");
    }

    #[test]
    fn the_search_for_a_frequency_gives_the_nearest_values() {
        let mut out = Vec::new();
        print_nearest("440", &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();

        assert!(text.starts_with("440.00 Hz."), "{text}");
        assert!(text.lines().nth(2).unwrap().trim_start().starts_with("88 "));
    }

    #[test]
    fn the_search_for_a_wrong_name_is_an_error() {
        let mut out = Vec::new();
        assert_eq!(
            print_nearest("H4", &mut out),
            Err("Bad note: H4. Give a note such as C4 or F#3, or a frequency in Hz".to_string())
        );
    }

    /// Run the play list with `keys`. Each key read advances the engine by
    /// one tick, as an output does. Give the pitches that started and the
    /// text.
    fn play_keys(keys: &[u8]) -> (Vec<u8>, String) {
        use crate::sound::{lock, SharedEngine, SoundEngine};
        use std::sync::{Arc, Mutex};

        let engine: SharedEngine = Arc::new(Mutex::new(SoundEngine::new(10_000)));
        lock(&engine).record_notes();
        let mut keys = keys.iter().copied();
        let clock = Arc::clone(&engine);
        let read_key = move || {
            lock(&clock).fill(&mut [0.0f32; 100]);
            keys.next()
        };
        let mut out = Vec::new();
        play_list(read_key, &mut out, &engine).unwrap();
        lock(&engine).fill(&mut [0.0f32; 100]);
        let pitches = lock(&engine)
            .take_notes()
            .iter()
            .map(|note| note.pitch)
            .collect();
        (pitches, String::from_utf8(out).unwrap())
    }

    #[test]
    fn the_play_list_starts_at_c4_and_plays_it_on_enter() {
        let (pitches, text) = play_keys(b"\rq");
        assert_eq!(pitches, vec![52]);
        assert!(text.contains("C4"), "{text}");
        assert!(text.contains("Up and Down"), "{text}");
    }

    #[test]
    fn the_up_and_down_keys_select_the_next_note() {
        // Up, Up, Enter: D4. Down, Enter: C#4. Down, Down, Enter: B3.
        let (pitches, _) = play_keys(b"\x1b[A\x1b[A\r\x1b[B\r\x1b[B\x1b[B\r");
        let expected: Vec<u8> = ["D4", "C#4", "B3"]
            .iter()
            .map(|name| note_pitch(name).unwrap())
            .collect();
        assert_eq!(pitches, expected);
    }

    #[test]
    fn the_play_list_ends_at_the_end_of_input_and_on_escape() {
        assert_eq!(play_keys(b"").0, Vec::<u8>::new());
        assert_eq!(play_keys(b"\x1b\r").0, Vec::<u8>::new());
    }

    #[test]
    fn the_table_has_a_header_and_256_rows() {
        let mut out = Vec::new();
        print_table(&mut out).unwrap();
        let text = String::from_utf8(out).unwrap();

        assert!(text.contains("quarter of a semitone sharp"));
        let rows: Vec<&str> = text.lines().filter(|line| line.contains(" Hz ")).collect();
        assert_eq!(rows.len(), 256);
        assert!(rows[52].ends_with('*'), "{}", rows[52]);
        assert!(!rows[53].ends_with('*'), "{}", rows[53]);
    }
}
