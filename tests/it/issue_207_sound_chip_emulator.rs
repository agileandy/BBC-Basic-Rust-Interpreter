//! Issue #207: a model of the SN76489 sound chip of the BBC Micro Model B.
//! The chip makes a buffer of mono samples, with no audio device.

use bbc_basic_interpreter::sound::{divider, frequency, Chip, CHIP_TICK_HZ, DEFAULT_SAMPLE_RATE};

/// The frequency of a buffer, from a count of its sign changes. A sample
/// of 0.0 does not change the sign.
fn measured_frequency(samples: &[f32], sample_rate: u32) -> f64 {
    let mut last = 0.0f32;
    let mut changes = 0u32;
    for &s in samples {
        if s != 0.0 {
            if last != 0.0 && (s > 0.0) != (last > 0.0) {
                changes += 1;
            }
            last = s;
        }
    }
    let seconds = samples.len() as f64 / sample_rate as f64;
    f64::from(changes) / 2.0 / seconds
}

/// One second of SOUND channel 1 at a pitch value and level 15.
fn render_pitch(pitch: u8) -> Vec<f32> {
    let mut chip = Chip::default();
    chip.set_tone(1, divider(1, pitch), 15);
    let mut buf = vec![0.0f32; DEFAULT_SAMPLE_RATE as usize];
    chip.render(&mut buf);
    buf
}

fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0f32, |m, s| m.max(s.abs()))
}

/// True if the buffer repeats with this period for its full length.
fn repeats_with(samples: &[f32], period: usize) -> bool {
    (0..samples.len() - period).all(|i| samples[i] == samples[i + period])
}

/// A chip with one sample for each tick of the chip, so a period in ticks
/// is the same period in samples.
fn tick_chip() -> Chip {
    Chip::new(CHIP_TICK_HZ)
}

#[test]
fn the_default_sample_rate_is_44100() {
    assert_eq!(DEFAULT_SAMPLE_RATE, 44_100);
    assert_eq!(Chip::default().sample_rate(), 44_100);
    assert_eq!(Chip::new(22_050).sample_rate(), 22_050);
}

#[test]
fn the_pitch_map_follows_the_mos_table() {
    // Pitch 0 is the B entry of the MOS table: divider 1008.
    assert_eq!(divider(1, 0), 1008);
    // Pitch 4 is the C entry: divider 951.
    assert_eq!(divider(1, 4), 951);
    // Pitch 53: octave 1, C, 1 quarter step: (951 - 13) / 2.
    assert_eq!(divider(1, 53), 469);
    // Pitch 89: octave 1, A, 1 quarter step: (565 - 8) / 2.
    assert_eq!(divider(1, 89), 278);
    // Pitch 255: octave 5, D, 3 quarter steps: (847 - 36) / 32.
    assert_eq!(divider(1, 255), 25);
}

#[test]
fn the_pitch_map_adds_the_offset_of_the_channel() {
    // The MOS adds 0, 0, 1 and 2 for channels 0, 1, 2 and 3.
    assert_eq!(divider(0, 53), 469);
    assert_eq!(divider(2, 53), 470);
    assert_eq!(divider(3, 53), 471);
}

#[test]
fn the_frequency_of_a_divider_is_the_clock_over_32_times_the_divider() {
    assert!((frequency(469) - 4_000_000.0 / (32.0 * 469.0)).abs() < 1e-9);
    assert!((frequency(1) - 125_000.0).abs() < 1e-9);
    // The chip uses a divider of 0 as 1024.
    assert!((frequency(0) - 125_000.0 / 1024.0).abs() < 1e-9);
}

#[test]
fn pitch_53_makes_266_hz_on_the_mos() {
    // The User Guide calls pitch 53 middle C (261.63 Hz). The MOS table
    // gives divider 469, which is 266.52 Hz.
    let f = measured_frequency(&render_pitch(53), DEFAULT_SAMPLE_RATE);
    assert!((f - 266.52).abs() < 1.0, "measured {f} Hz");
}

#[test]
fn pitch_89_makes_450_hz_on_the_mos() {
    // The User Guide calls pitch 89 the A of 440 Hz. The MOS table gives
    // divider 278, which is 449.64 Hz.
    let f = measured_frequency(&render_pitch(89), DEFAULT_SAMPLE_RATE);
    assert!((f - 449.64).abs() < 1.0, "measured {f} Hz");
}

#[test]
fn pitch_255_makes_5000_hz() {
    let f = measured_frequency(&render_pitch(255), DEFAULT_SAMPLE_RATE);
    assert!((f - 5000.0).abs() < 1.0, "measured {f} Hz");
}

#[test]
fn a_second_render_continues_the_wave_with_no_step() {
    let mut one = Chip::default();
    one.set_tone(1, 469, 15);
    let mut whole = vec![0.0f32; 1000];
    one.render(&mut whole);

    let mut two = Chip::default();
    two.set_tone(1, 469, 15);
    let mut first = vec![0.0f32; 377];
    let mut second = vec![0.0f32; 623];
    two.render(&mut first);
    two.render(&mut second);

    assert_eq!(&whole[..377], &first[..]);
    assert_eq!(&whole[377..], &second[..]);
}

#[test]
fn level_0_is_silence() {
    let mut chip = Chip::default();
    chip.set_tone(1, 469, 0);
    chip.set_tone(2, 300, 0);
    chip.set_tone(3, 200, 0);
    chip.set_noise(4, 0);
    let mut buf = vec![1.0f32; 4410];
    chip.render(&mut buf);
    assert!(buf.iter().all(|&s| s == 0.0));
}

#[test]
fn a_new_chip_is_silent() {
    let mut chip = Chip::default();
    let mut buf = vec![1.0f32; 4410];
    chip.render(&mut buf);
    assert!(buf.iter().all(|&s| s == 0.0));
}

#[test]
fn each_level_step_is_2_db_and_level_15_is_the_loudest() {
    let peaks: Vec<f32> = (0..=15u8)
        .map(|level| {
            let mut chip = tick_chip();
            chip.set_tone(1, 100, level);
            let mut buf = vec![0.0f32; 1000];
            chip.render(&mut buf);
            peak(&buf)
        })
        .collect();
    assert_eq!(peaks[0], 0.0);
    for level in 2..=15 {
        let db = 20.0 * (f64::from(peaks[level]) / f64::from(peaks[level - 1])).log10();
        assert!((db - 2.0).abs() < 0.01, "level {level}: step of {db} dB");
    }
    assert_eq!(peaks[15], peaks.iter().cloned().fold(0.0, f32::max));
}

#[test]
fn set_level_changes_the_level_and_keeps_the_divider() {
    let mut chip = tick_chip();
    chip.set_tone(1, 100, 15);
    chip.set_level(1, 13);
    let mut buf = vec![0.0f32; 1000];
    chip.render(&mut buf);
    let mut reference = tick_chip();
    reference.set_tone(1, 100, 13);
    let mut expected = vec![0.0f32; 1000];
    reference.render(&mut expected);
    assert_eq!(buf, expected);
}

#[test]
fn periodic_noise_repeats_with_the_15_bit_register() {
    // Setting 0 shifts the register each 32 ticks. Periodic noise has a
    // cycle of 15 shifts: 480 ticks.
    let mut chip = tick_chip();
    chip.set_noise(0, 15);
    let mut buf = vec![0.0f32; 4800];
    chip.render(&mut buf);
    assert!(repeats_with(&buf, 480));
    assert!(!repeats_with(&buf, 240));
    assert!(!repeats_with(&buf, 32));
}

#[test]
fn the_periodic_noise_rates_are_the_clock_over_512_1024_and_2048() {
    for (setting, period) in [(0u8, 480usize), (1, 960), (2, 1920)] {
        let mut chip = tick_chip();
        chip.set_noise(setting, 15);
        let mut buf = vec![0.0f32; period * 4];
        chip.render(&mut buf);
        assert!(repeats_with(&buf, period), "setting {setting}");
        assert!(!repeats_with(&buf, period / 2), "setting {setting}");
    }
}

#[test]
fn white_noise_does_not_repeat() {
    let mut chip = tick_chip();
    chip.set_noise(4, 15);
    let mut buf = vec![0.0f32; 16_384];
    chip.render(&mut buf);
    for period in 1..=8192 {
        assert!(
            !repeats_with(&buf[..period + 8192], period),
            "white noise repeats with period {period}"
        );
    }
}

#[test]
fn white_noise_has_both_signs() {
    let mut chip = tick_chip();
    chip.set_noise(4, 15);
    let mut buf = vec![0.0f32; 16_384];
    chip.render(&mut buf);
    assert!(buf.iter().any(|&s| s > 0.0));
    assert!(buf.iter().any(|&s| s < 0.0));
}

#[test]
fn noise_setting_3_follows_tone_channel_1() {
    // A divider of D makes a square wave of 2 D ticks. The noise register
    // shifts once for each wave, so periodic noise has a cycle of 30 D.
    for d in [50u16, 100] {
        let mut chip = tick_chip();
        // Channel 1 is silent, but its wave still drives the noise.
        chip.set_tone(1, d, 0);
        chip.set_noise(3, 15);
        let mut buf = vec![0.0f32; 30 * d as usize * 4];
        chip.render(&mut buf);
        assert!(repeats_with(&buf, 30 * d as usize), "divider {d}");
        assert!(!repeats_with(&buf, 480), "divider {d}");
    }
}

#[test]
fn noise_setting_7_is_white_noise_at_the_rate_of_tone_channel_1() {
    let mut chip = tick_chip();
    chip.set_tone(1, 10, 0);
    chip.set_noise(7, 15);
    let mut buf = vec![0.0f32; 8192];
    chip.render(&mut buf);
    // The register shifts each 20 ticks, so the output holds for 20 ticks.
    assert!(buf.chunks(20).all(|c| c.iter().all(|&s| s == c[0])));
    assert!(!repeats_with(&buf, 300));
}

#[test]
fn the_mixer_does_not_clip_with_4_channels_at_level_15() {
    let mut chip = Chip::default();
    chip.set_tone(1, 469, 15);
    chip.set_tone(2, 278, 15);
    chip.set_tone(3, 25, 15);
    chip.set_noise(4, 15);
    let mut buf = vec![0.0f32; DEFAULT_SAMPLE_RATE as usize];
    chip.render(&mut buf);
    let p = peak(&buf);
    assert!(p <= 1.0, "peak {p}");
    // The 4 channels add: the peak is more than the peak of 3 channels.
    assert!(p > 0.75, "peak {p}");
}

#[test]
fn one_channel_at_level_15_has_a_quarter_of_the_full_scale() {
    let mut chip = tick_chip();
    chip.set_tone(2, 100, 15);
    let mut buf = vec![0.0f32; 1000];
    chip.render(&mut buf);
    assert_eq!(peak(&buf), 0.25);
}

#[test]
#[should_panic(expected = "tone channel")]
fn set_tone_rejects_the_noise_channel() {
    Chip::default().set_tone(0, 100, 15);
}

#[test]
#[should_panic(expected = "level")]
fn set_tone_rejects_a_level_above_15() {
    Chip::default().set_tone(1, 100, 16);
}

#[test]
#[should_panic(expected = "noise setting")]
fn set_noise_rejects_a_setting_above_7() {
    Chip::default().set_noise(8, 15);
}
