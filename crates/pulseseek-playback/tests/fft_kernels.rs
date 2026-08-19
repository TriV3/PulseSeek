use std::f32::consts::TAU;

use pulseseek_domain::analysis_subscriptions::WindowFunction;
use pulseseek_playback::{FftKernel, SUPPORTED_FFT_SIZES, SUPPORTED_WINDOWS};

const SAMPLE_RATE: u32 = 48_000;

fn sine(size: usize, bin: f32, amplitude: f32) -> Vec<f32> {
    (0..size).map(|index| amplitude * (TAU * bin * index as f32 / size as f32).sin()).collect()
}

fn amplitude_db(actual: f32, expected: f32) -> f32 {
    20.0 * (actual / expected).log10()
}

#[test]
fn supports_normative_sizes_and_windows_with_calibrated_amplitude() {
    for size in SUPPORTED_FFT_SIZES {
        for window in SUPPORTED_WINDOWS {
            let mut kernel = FftKernel::new(size, window).unwrap();
            let analysis = kernel.analyze(&sine(size, 32.0, 0.125), SAMPLE_RATE).unwrap();

            assert_eq!(analysis.amplitudes().len(), size / 2 + 1);
            assert_eq!(analysis.bin_frequency_hz(32), 32.0 * SAMPLE_RATE as f32 / size as f32);
            assert!(
                amplitude_db(analysis.amplitudes()[32], 0.125).abs() <= 0.1,
                "size={size}, window={window:?}, amplitude={}",
                analysis.amplitudes()[32]
            );
        }
    }
}

#[test]
fn publishes_window_normalization_values() {
    let expectations = [
        (WindowFunction::Rectangular, 1.0, 1.0),
        (WindowFunction::Hann, 0.5, 0.375),
        (WindowFunction::Hamming, 0.54, 0.3974),
        (WindowFunction::BlackmanHarris, 0.42323, 0.306_04),
        (WindowFunction::FlatTop, 0.215_578_95, 0.175_22),
    ];

    for (window, coherent_gain, mean_square) in expectations {
        let kernel = FftKernel::new(16_384, window).unwrap();
        assert!((kernel.coherent_gain() - coherent_gain).abs() < 0.0001, "{window:?}");
        assert!(
            (kernel.power_normalization() / 16_384.0 - mean_square).abs() < 0.0001,
            "{window:?}: {}",
            kernel.power_normalization() / 16_384.0
        );
    }
}

#[test]
fn power_and_psd_integrate_to_sine_mean_square() {
    let size = 4_096;
    let mut kernel = FftKernel::new(size, WindowFunction::Hann).unwrap();
    let analysis = kernel.analyze(&sine(size, 64.0, 0.5), SAMPLE_RATE).unwrap();
    let total_power: f32 = analysis.powers().iter().sum();
    let integrated_psd: f32 = analysis.psd().iter().sum::<f32>() * analysis.bin_width_hz();

    assert!((total_power - 0.125).abs() < 0.0001);
    assert!((integrated_psd - total_power).abs() < 0.0001);
}

#[test]
fn equivalent_noise_bandwidth_matches_each_window() {
    let expected_bin_widths = [
        (WindowFunction::Rectangular, 1.0),
        (WindowFunction::Hann, 1.5),
        (WindowFunction::Hamming, 1.362_83),
        (WindowFunction::BlackmanHarris, 1.708_54),
        (WindowFunction::FlatTop, 3.770_25),
    ];

    for (window, expected_bins) in expected_bin_widths {
        let kernel = FftKernel::new(16_384, window).unwrap();
        let actual_bins = kernel.equivalent_noise_bandwidth_hz(SAMPLE_RATE).unwrap()
            / (SAMPLE_RATE as f32 / 16_384.0);
        assert!((actual_bins - expected_bins).abs() < 0.001, "{window:?}: {actual_bins}");
    }

    assert_eq!(
        FftKernel::new(2_048, WindowFunction::Hann).unwrap().equivalent_noise_bandwidth_hz(0),
        Err(pulseseek_playback::FftError::InvalidSampleRate)
    );
}

#[test]
fn flat_top_preserves_off_bin_amplitude() {
    let size = 4_096;
    let mut kernel = FftKernel::new(size, WindowFunction::FlatTop).unwrap();
    let analysis = kernel.analyze(&sine(size, 64.5, 0.25), SAMPLE_RATE).unwrap();
    let peak = analysis.amplitudes().iter().copied().fold(0.0, f32::max);

    assert!(amplitude_db(peak, 0.25).abs() <= 0.1);
}

#[test]
fn dc_and_nyquist_are_not_doubled() {
    let size = 2_048;
    let mut kernel = FftKernel::new(size, WindowFunction::Rectangular).unwrap();
    let dc = kernel.analyze(&vec![0.25; size], SAMPLE_RATE).unwrap().amplitudes()[0];
    let nyquist_samples: Vec<f32> =
        (0..size).map(|index| if index % 2 == 0 { 0.25 } else { -0.25 }).collect();
    let nyquist = kernel.analyze(&nyquist_samples, SAMPLE_RATE).unwrap().amplitudes()[size / 2];

    assert!((dc - 0.25).abs() < 0.0001);
    assert!((nyquist - 0.25).abs() < 0.0001);
}

#[test]
fn blackman_harris_reduces_off_bin_leakage() {
    let size = 4_096;
    let signal = sine(size, 64.25, 0.5);
    let mut rectangular = FftKernel::new(size, WindowFunction::Rectangular).unwrap();
    let rectangular_leakage = rectangular
        .analyze(&signal, SAMPLE_RATE)
        .unwrap()
        .amplitudes()
        .iter()
        .enumerate()
        .filter(|(index, _)| !(61..=67).contains(index))
        .map(|(_, value)| *value)
        .fold(0.0, f32::max);
    let mut blackman_harris = FftKernel::new(size, WindowFunction::BlackmanHarris).unwrap();
    let blackman_harris_leakage = blackman_harris
        .analyze(&signal, SAMPLE_RATE)
        .unwrap()
        .amplitudes()
        .iter()
        .enumerate()
        .filter(|(index, _)| !(61..=67).contains(index))
        .map(|(_, value)| *value)
        .fold(0.0, f32::max);

    assert!(blackman_harris_leakage < rectangular_leakage * 0.1);
}

#[test]
fn rejects_non_normative_sizes_and_invalid_input() {
    assert!(FftKernel::new(1_024, WindowFunction::Hann).is_err());
    let mut kernel = FftKernel::new(2_048, WindowFunction::Hann).unwrap();
    assert!(kernel.analyze(&[0.0; 1_024], SAMPLE_RATE).is_err());
    let mut invalid = vec![0.0; 2_048];
    invalid[0] = f32::NAN;
    assert!(kernel.analyze(&invalid, SAMPLE_RATE).is_err());
    assert!(kernel.analyze(&[0.0; 2_048], 0).is_err());
}
