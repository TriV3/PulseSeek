use std::f32::consts::TAU;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use pulseseek_domain::visualization::VisualizationFrame;
use pulseseek_playback::{visualization_channel, FftAnalyzer, FftError, FftWorker, PublishResult};

const FFT_SIZE: usize = 1_024;
const SAMPLE_RATE: u32 = 48_000;

fn signal_frame(sequence: u64, channels: u16, tones: &[(usize, f32)]) -> VisualizationFrame {
    let mut samples = Vec::with_capacity(FFT_SIZE * usize::from(channels));
    for sample_index in 0..FFT_SIZE {
        let value = tones.iter().fold(0.0, |sum, (bin, amplitude)| {
            sum + amplitude * (TAU * *bin as f32 * sample_index as f32 / FFT_SIZE as f32).sin()
        });
        for _ in 0..channels {
            samples.push(value);
        }
    }
    VisualizationFrame::new(sequence, sequence * FFT_SIZE as u64, SAMPLE_RATE, channels, &samples)
        .unwrap()
}

fn assert_close(actual: f32, expected: f32, tolerance: f32) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected} +/- {tolerance}, got {actual}"
    );
}

#[test]
fn known_tone_peaks_at_the_expected_frequency_bin() {
    let mut analyzer = FftAnalyzer::new(FFT_SIZE).unwrap();
    let spectrum = analyzer.analyze(&signal_frame(4, 1, &[(32, 0.8)])).unwrap();

    assert_eq!(spectrum.sequence(), 4);
    assert_eq!(spectrum.position_frames(), 4 * FFT_SIZE as u64);
    assert_close(spectrum.bin_frequency_hz(32).unwrap(), 1_500.0, f32::EPSILON);
    assert_close(spectrum.magnitudes()[32], 0.8, 0.01);
    assert_eq!(
        spectrum
            .magnitudes()
            .iter()
            .enumerate()
            .max_by(|left, right| left.1.total_cmp(right.1))
            .map(|(index, _)| index),
        Some(32)
    );
}

#[test]
fn silence_produces_zero_magnitudes() {
    let mut analyzer = FftAnalyzer::new(FFT_SIZE).unwrap();
    let spectrum = analyzer.analyze(&signal_frame(0, 1, &[])).unwrap();

    assert!(spectrum.magnitudes().iter().all(|magnitude| magnitude.abs() <= 1.0e-7));
}

#[test]
fn mixed_tones_and_interleaved_channels_preserve_expected_amplitudes() {
    let mut analyzer = FftAnalyzer::new(FFT_SIZE).unwrap();
    let spectrum = analyzer.analyze(&signal_frame(0, 2, &[(20, 0.75), (75, 0.25)])).unwrap();

    assert_close(spectrum.magnitudes()[20], 0.75, 0.01);
    assert_close(spectrum.magnitudes()[75], 0.25, 0.01);
}

#[test]
fn interleaved_channels_are_averaged_before_analysis() {
    let mut samples = Vec::with_capacity(FFT_SIZE * 2);
    for sample_index in 0..FFT_SIZE {
        let left = 0.8 * (TAU * 40.0 * sample_index as f32 / FFT_SIZE as f32).sin();
        samples.extend([left, 0.0]);
    }
    let frame = VisualizationFrame::new(0, 0, SAMPLE_RATE, 2, &samples).unwrap();
    let mut analyzer = FftAnalyzer::new(FFT_SIZE).unwrap();

    let spectrum = analyzer.analyze(&frame).unwrap();

    assert_close(spectrum.magnitudes()[40], 0.4, 0.01);
}

#[test]
fn analyzer_rejects_frames_with_a_different_fft_size() {
    let mut analyzer = FftAnalyzer::new(FFT_SIZE).unwrap();
    let short = VisualizationFrame::new(0, 0, SAMPLE_RATE, 1, &[0.0; 512]).unwrap();

    assert_eq!(
        analyzer.analyze(&short).unwrap_err(),
        FftError::FrameSizeMismatch { expected: FFT_SIZE, actual: 512 }
    );
}

#[test]
fn analyzer_and_worker_reject_invalid_configuration() {
    assert!(matches!(FftAnalyzer::new(0), Err(FftError::InvalidFftSize { requested: 0 })));
    assert!(matches!(FftAnalyzer::new(3), Err(FftError::InvalidFftSize { requested: 3 })));
    assert!(matches!(
        FftAnalyzer::new(16_384),
        Err(FftError::InvalidFftSize { requested: 16_384 })
    ));

    let (_publisher, subscriber) = visualization_channel(1);
    assert!(matches!(
        FftWorker::start(subscriber, FFT_SIZE, 0),
        Err(FftError::InvalidOutputCapacity)
    ));
}

#[test]
fn worker_discards_stale_input_and_analyzes_the_latest_frame() {
    let (mut publisher, subscriber) = visualization_channel(4);
    for sequence in 1..=3 {
        assert_eq!(
            publisher.try_publish(signal_frame(sequence, 1, &[(sequence as usize * 10, 0.5)])),
            PublishResult::Published
        );
    }

    let (worker, spectra) = FftWorker::start(subscriber, FFT_SIZE, 1).unwrap();
    let spectrum = spectra.recv_timeout(Duration::from_secs(1)).unwrap();

    assert_eq!(spectrum.sequence(), 3);
    assert_close(spectrum.magnitudes()[30], 0.5, 0.01);
    assert_eq!(worker.skipped_input_frames(), 2);

    publisher.shutdown();
    assert_eq!(worker.wait(), Ok(()));
}

#[test]
fn worker_cancellation_is_deterministic() {
    let (_publisher, subscriber) = visualization_channel(1);
    let (worker, _spectra) = FftWorker::start(subscriber, FFT_SIZE, 1).unwrap();

    worker.cancel();

    assert_eq!(worker.wait(), Err(FftError::Cancelled));
}

#[test]
fn dropping_spectrum_receiver_stops_an_idle_worker() {
    let (publisher, subscriber) = visualization_channel(1);
    let (worker, spectra) = FftWorker::start(subscriber, FFT_SIZE, 1).unwrap();
    let (finished_tx, finished_rx) = mpsc::sync_channel(1);
    let waiter = thread::spawn(move || {
        finished_tx.send(worker.wait()).unwrap();
    });

    drop(spectra);

    let stopped_without_more_input = finished_rx.recv_timeout(Duration::from_secs(1));
    if stopped_without_more_input.is_err() {
        publisher.shutdown();
        let _ = finished_rx.recv_timeout(Duration::from_secs(1));
    }
    waiter.join().unwrap();
    assert_eq!(stopped_without_more_input, Ok(Ok(())));
}
