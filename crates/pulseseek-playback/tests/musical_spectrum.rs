use std::f32::consts::TAU;

use pulseseek_domain::visualization::VisualizationFrame;
use pulseseek_playback::{
    FftAnalyzer, MusicalSpectrumAnalyzer, MusicalSpectrumError, DEFAULT_TUNING_REFERENCE_HZ,
};

const FFT_SIZE: usize = 4_096;
const SAMPLE_RATE: u32 = 48_000;

fn sine_frame(frequency_hz: f32) -> VisualizationFrame {
    let samples: Vec<f32> = (0..FFT_SIZE)
        .map(|index| (TAU * frequency_hz * index as f32 / SAMPLE_RATE as f32).sin())
        .collect();
    VisualizationFrame::new(7, 12_288, SAMPLE_RATE, 1, &samples).unwrap()
}

fn musical_spectrum(frequency_hz: f32) -> pulseseek_domain::visualization::MusicalSpectrumFrame {
    let spectrum = FftAnalyzer::new(FFT_SIZE).unwrap().analyze(&sine_frame(frequency_hz)).unwrap();
    MusicalSpectrumAnalyzer::new(DEFAULT_TUNING_REFERENCE_HZ).unwrap().analyze(&spectrum).unwrap()
}

#[test]
fn tuning_reference_defines_pitch_centres_and_contiguous_boundaries() {
    let spectrum = FftAnalyzer::new(FFT_SIZE).unwrap().analyze(&sine_frame(440.0)).unwrap();
    let frame = MusicalSpectrumAnalyzer::new(440.0).unwrap().analyze(&spectrum).unwrap();

    let a4 = frame.bands().iter().find(|band| band.note_number() == 69).unwrap();
    let g_sharp4 = frame.bands().iter().find(|band| band.note_number() == 68).unwrap();
    let half_semitone = 2_f32.powf(1.0 / 24.0);

    assert!((a4.center_frequency_hz() - 440.0).abs() < 0.001);
    assert!((a4.lower_frequency_hz() - 440.0 / half_semitone).abs() < 0.001);
    assert!((a4.upper_frequency_hz() - 440.0 * half_semitone).abs() < 0.001);
    assert!((g_sharp4.upper_frequency_hz() - a4.lower_frequency_hz()).abs() < 0.001);

    let concert_pitch = MusicalSpectrumAnalyzer::new(442.0).unwrap().analyze(&spectrum).unwrap();
    let tuned_a4 = concert_pitch.bands().iter().find(|band| band.note_number() == 69).unwrap();
    assert!((tuned_a4.center_frequency_hz() - 442.0).abs() < 0.001);
}

#[test]
fn known_sine_tones_peak_in_their_expected_musical_bands() {
    for (frequency_hz, expected_note) in [(110.0, 45), (440.0, 69), (523.251_1, 72)] {
        let frame = musical_spectrum(frequency_hz);
        let strongest = frame
            .bands()
            .iter()
            .max_by(|left, right| left.magnitude().total_cmp(&right.magnitude()))
            .unwrap();

        assert_eq!(
            strongest.note_number(),
            expected_note,
            "{frequency_hz} Hz should peak in note {expected_note}"
        );
    }
}

#[test]
fn silence_produces_zero_energy_in_every_musical_band() {
    let silence = VisualizationFrame::new(1, 0, SAMPLE_RATE, 1, &[0.0; FFT_SIZE]).unwrap();
    let spectrum = FftAnalyzer::new(FFT_SIZE).unwrap().analyze(&silence).unwrap();
    let frame = MusicalSpectrumAnalyzer::new(DEFAULT_TUNING_REFERENCE_HZ)
        .unwrap()
        .analyze(&spectrum)
        .unwrap();

    assert!(frame.bands().iter().all(|band| band.magnitude() == 0.0));
    assert!(frame.bands().iter().all(|band| band.center_frequency_hz() < SAMPLE_RATE as f32 / 2.0));
}

#[test]
fn invalid_tuning_references_are_rejected() {
    for tuning in [0.0, -440.0, f32::NAN, f32::INFINITY] {
        assert_eq!(
            MusicalSpectrumAnalyzer::new(tuning).unwrap_err(),
            MusicalSpectrumError::InvalidTuningReference
        );
    }
}
