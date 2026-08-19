use std::f32::consts::{SQRT_2, TAU};

use pulseseek_domain::analysis_subscriptions::{ChannelMode, WindowFunction};
use pulseseek_playback::{FftBank, FftBranchKey, FftError, FftStreamId, SUPPORTED_FFT_SIZES};

const SAMPLE_RATE: u32 = 48_000;
const STREAM: FftStreamId = FftStreamId::new(7);

fn bank() -> FftBank {
    FftBank::new(STREAM)
}

fn stereo_sines(
    size: usize,
    left_amplitude: f32,
    right_amplitude: f32,
    inverted: bool,
) -> Vec<f32> {
    (0..size)
        .flat_map(|index| {
            let phase = (TAU * 32.0 * index as f32 / size as f32).sin();
            let right_sign = if inverted { -1.0 } else { 1.0 };
            [left_amplitude * phase, right_sign * right_amplitude * phase]
        })
        .collect()
}

fn peak(frame: &pulseseek_playback::FftBankAnalysis<'_>, mode: ChannelMode) -> f32 {
    frame.amplitudes(mode).unwrap()[32]
}

#[test]
fn stream_identity_is_explicit_for_overlapping_frame_sequences() {
    let mut source = FftBank::new(FftStreamId::new(1));
    let mut monitor = FftBank::new(FftStreamId::new(2));
    let key = FftBranchKey::new(2_048, WindowFunction::Hann);
    let source_subscription = source.subscribe(key).unwrap();
    let monitor_subscription = monitor.subscribe(key).unwrap();

    source
        .process(source_subscription.id(), 1, &stereo_sines(2_048, 0.25, 0.5, false), SAMPLE_RATE)
        .unwrap();
    monitor
        .process(monitor_subscription.id(), 1, &stereo_sines(2_048, 0.75, 0.1, false), SAMPLE_RATE)
        .unwrap();

    assert_eq!(source.analysis(source_subscription.id()).unwrap().stream_id(), FftStreamId::new(1));
    assert_eq!(
        monitor.analysis(monitor_subscription.id()).unwrap().stream_id(),
        FftStreamId::new(2)
    );
}

#[test]
fn analysis_exposes_frame_identity_and_sample_rate_transitions() {
    let mut bank = bank();
    let subscription = bank.subscribe(FftBranchKey::new(2_048, WindowFunction::Hann)).unwrap();
    let samples = stereo_sines(2_048, 0.25, 0.5, false);

    bank.process(subscription.id(), 41, &samples, 44_100).unwrap();
    let first = bank.analysis(subscription.id()).unwrap();
    assert_eq!(first.frame_id(), 41);
    assert_eq!(first.sample_rate(), 44_100);
    assert!((first.bin_frequency_hz(32) - 689.0625).abs() < 0.001);

    bank.process(subscription.id(), 42, &samples, 96_000).unwrap();
    let second = bank.analysis(subscription.id()).unwrap();
    assert_eq!(second.frame_id(), 42);
    assert_eq!(second.sample_rate(), 96_000);
    assert_eq!(second.bin_frequency_hz(32), 1_500.0);
}

#[test]
fn supports_all_normative_sizes_concurrently() {
    let mut bank = bank();
    let subscriptions: Vec<_> = SUPPORTED_FFT_SIZES
        .into_iter()
        .map(|size| bank.subscribe(FftBranchKey::new(size, WindowFunction::Hann)).unwrap())
        .collect();

    assert_eq!(bank.active_branch_count(), 4);
    assert_eq!(bank.active_plan_count(), 8);
    for (size, subscription) in SUPPORTED_FFT_SIZES.into_iter().zip(&subscriptions) {
        bank.process(
            subscription.id(),
            size as u64,
            &stereo_sines(size, 0.25, 0.5, false),
            SAMPLE_RATE,
        )
        .unwrap();
        let frame = bank.analysis(subscription.id()).unwrap();
        assert_eq!(frame.fft_size(), size);
        assert_eq!(frame.amplitudes(ChannelMode::Left).unwrap().len(), size / 2 + 1);
    }
}

#[test]
fn produces_calibrated_channel_transforms() {
    let mut bank = bank();
    let subscription = bank.subscribe(FftBranchKey::new(2_048, WindowFunction::Hann)).unwrap();
    bank.process(subscription.id(), 1, &stereo_sines(2_048, 0.25, 0.5, false), SAMPLE_RATE)
        .unwrap();
    let frame = bank.analysis(subscription.id()).unwrap();

    assert!((peak(&frame, ChannelMode::Left) - 0.25).abs() < 0.001);
    assert!((peak(&frame, ChannelMode::Right) - 0.5).abs() < 0.001);
    assert!((peak(&frame, ChannelMode::Mono) - 0.375).abs() < 0.001);
    assert!((peak(&frame, ChannelMode::Mid) - 0.75 / SQRT_2).abs() < 0.001);
    assert!((peak(&frame, ChannelMode::Side) - 0.25 / SQRT_2).abs() < 0.001);
    assert!((peak(&frame, ChannelMode::EnergySum) - 0.15625_f32.sqrt()).abs() < 0.001);
}

#[test]
fn phase_sensitive_transforms_preserve_cancellation() {
    let mut bank = bank();
    let subscription =
        bank.subscribe(FftBranchKey::new(2_048, WindowFunction::Rectangular)).unwrap();
    bank.process(subscription.id(), 1, &stereo_sines(2_048, 0.5, 0.5, true), SAMPLE_RATE).unwrap();
    let frame = bank.analysis(subscription.id()).unwrap();

    assert!(peak(&frame, ChannelMode::Mono) < 0.0001);
    assert!(peak(&frame, ChannelMode::Mid) < 0.0001);
    assert!((peak(&frame, ChannelMode::Side) - 1.0 / SQRT_2).abs() < 0.001);
    assert!((peak(&frame, ChannelMode::EnergySum) - 0.5).abs() < 0.001);
}

#[test]
fn compatible_subscribers_reuse_branch_until_last_release() {
    let mut bank = bank();
    let key = FftBranchKey::new(4_096, WindowFunction::Hann);
    let first = bank.subscribe(key).unwrap();
    let second = bank.subscribe(key).unwrap();

    assert_eq!(first.branch_id(), second.branch_id());
    assert_eq!(bank.active_branch_count(), 1);
    assert_eq!(bank.consumer_count(key), 2);
    assert!(bank.unsubscribe(first.id()));
    assert_eq!(bank.active_branch_count(), 1);
    assert_eq!(bank.consumer_count(key), 1);
    assert!(bank.unsubscribe(second.id()));
    assert_eq!(bank.active_branch_count(), 0);
    assert_eq!(bank.active_plan_count(), 0);
    assert!(!bank.unsubscribe(second.id()));
}

#[test]
fn compatible_subscribers_transform_shared_frame_once() {
    let mut bank = bank();
    let key = FftBranchKey::new(2_048, WindowFunction::Hann);
    let first = bank.subscribe(key).unwrap();
    let second = bank.subscribe(key).unwrap();
    let samples = stereo_sines(2_048, 0.25, 0.5, false);

    bank.process(first.id(), 42, &samples, SAMPLE_RATE).unwrap();
    bank.process(second.id(), 42, &samples, SAMPLE_RATE).unwrap();

    assert_eq!(bank.branch_transform_count(key), 1);
    assert!((peak(&bank.analysis(first.id()).unwrap(), ChannelMode::Left) - 0.25).abs() < 0.001);
    assert!((peak(&bank.analysis(second.id()).unwrap(), ChannelMode::Right) - 0.5).abs() < 0.001);
}

#[test]
fn rejects_conflicting_or_stale_frame_identity_without_changing_result() {
    let mut bank = bank();
    let key = FftBranchKey::new(2_048, WindowFunction::Hann);
    let subscription = bank.subscribe(key).unwrap();
    let first = stereo_sines(2_048, 0.25, 0.5, false);
    let conflicting = stereo_sines(2_048, 0.75, 0.1, false);

    bank.process(subscription.id(), 42, &first, SAMPLE_RATE).unwrap();
    assert_eq!(
        bank.process(subscription.id(), 42, &conflicting, SAMPLE_RATE),
        Err(FftError::ConflictingFrameIdentity { frame_id: 42 })
    );
    assert_eq!(
        bank.process(subscription.id(), 41, &first, SAMPLE_RATE),
        Err(FftError::StaleFrame { latest: 42, received: 41 })
    );
    assert!(
        (peak(&bank.analysis(subscription.id()).unwrap(), ChannelMode::Left) - 0.25).abs() < 0.001
    );
    assert_eq!(bank.branch_transform_count(key), 1);
}

#[test]
fn validates_cached_frame_input_and_preserves_previous_result_after_failure() {
    let mut bank = bank();
    let subscription = bank.subscribe(FftBranchKey::new(2_048, WindowFunction::Hann)).unwrap();
    let first = stereo_sines(2_048, 0.25, 0.5, false);
    bank.process(subscription.id(), 42, &first, SAMPLE_RATE).unwrap();
    let mut invalid = stereo_sines(2_048, 0.75, 0.1, false);
    invalid[1] = f32::NAN;

    assert_eq!(
        bank.process(subscription.id(), 43, &invalid, SAMPLE_RATE),
        Err(FftError::NonFiniteInput)
    );
    assert_eq!(
        bank.process(subscription.id(), 42, &[0.0; 2_048], SAMPLE_RATE),
        Err(FftError::InterleavedFrameSizeMismatch { expected: 4_096, actual: 2_048 })
    );
    let analysis = bank.analysis(subscription.id()).unwrap();
    assert!((peak(&analysis, ChannelMode::Left) - 0.25).abs() < 0.001);
    assert!((peak(&analysis, ChannelMode::Right) - 0.5).abs() < 0.001);
}

#[test]
fn exposes_overlay_difference_and_balance_without_extra_transforms() {
    let mut bank = bank();
    let key = FftBranchKey::new(2_048, WindowFunction::Hann);
    let subscription = bank.subscribe(key).unwrap();
    bank.process(subscription.id(), 1, &stereo_sines(2_048, 0.25, 0.5, false), SAMPLE_RATE)
        .unwrap();

    let analysis = bank.analysis(subscription.id()).unwrap();
    let (left, right) = analysis.left_right_overlay();
    assert!((left[32] - 0.25).abs() < 0.001);
    assert!((right[32] - 0.5).abs() < 0.001);
    assert!((analysis.left_right_difference()[32] + 0.25).abs() < 0.001);
    assert!((analysis.left_right_balance()[32].unwrap() + 1.0 / 3.0).abs() < 0.001);
    assert_eq!(analysis.left_right_balance()[0], None);
    assert_eq!(bank.branch_transform_count(key), 1);
}

#[test]
fn incompatible_size_or_window_creates_smallest_branch() {
    let mut bank = bank();
    let hann = bank.subscribe(FftBranchKey::new(2_048, WindowFunction::Hann)).unwrap();
    let hamming = bank.subscribe(FftBranchKey::new(2_048, WindowFunction::Hamming)).unwrap();
    let larger = bank.subscribe(FftBranchKey::new(4_096, WindowFunction::Hann)).unwrap();

    assert_ne!(hann.branch_id(), hamming.branch_id());
    assert_ne!(hann.branch_id(), larger.branch_id());
    assert_eq!(bank.active_branch_count(), 3);
}

#[test]
fn rejects_invalid_frames_and_stale_subscriptions() {
    let mut bank = bank();
    let subscription = bank.subscribe(FftBranchKey::new(2_048, WindowFunction::Hann)).unwrap();

    assert!(matches!(bank.analysis(subscription.id()), Err(FftError::AnalysisUnavailable)));
    assert!(matches!(
        bank.process(subscription.id(), 1, &[0.0; 2_048], SAMPLE_RATE),
        Err(FftError::InterleavedFrameSizeMismatch { expected: 4_096, actual: 2_048 })
    ));
    assert!(bank.unsubscribe(subscription.id()));
    assert!(matches!(
        bank.process(subscription.id(), 2, &[0.0; 4_096], SAMPLE_RATE),
        Err(FftError::UnknownSubscription)
    ));
}
