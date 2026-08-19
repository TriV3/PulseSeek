use std::thread;

use pulseseek_domain::analysis::{AudioFormat, MeasurementPoint, SessionId, SourceId, SourceKind};
use pulseseek_playback::{
    analysis_capture_channel, AnalysisCaptureConfig, CaptureResult, MAX_ANALYSIS_CAPTURE_SAMPLES,
};

fn config(format: AudioFormat) -> AnalysisCaptureConfig {
    AnalysisCaptureConfig::new(
        SourceId::new("player"),
        SessionId::new("session-1"),
        SourceKind::Playback,
        MeasurementPoint::Source,
        format,
    )
}

#[test]
fn channel_enforces_exact_capacity_and_preserves_fifo_blocks() {
    let (mut producer, mut consumer) =
        analysis_capture_channel(2, config(AudioFormat::stereo(48_000).unwrap()));

    assert_eq!(producer.try_capture(10, &[0.1, 0.2], false), CaptureResult::Captured);
    assert_eq!(producer.try_capture(11, &[0.3, 0.4], false), CaptureResult::Captured);
    assert_eq!(producer.try_capture(12, &[0.5, 0.6], false), CaptureResult::DroppedFull);

    let first = consumer.try_receive().unwrap();
    let second = consumer.try_receive().unwrap();
    assert_eq!(first.first_sample(), 10);
    assert_eq!(first.sequence(), 0);
    assert_eq!(first.interleaved_samples(), &[0.1, 0.2]);
    assert_eq!(second.first_sample(), 11);
    assert_eq!(second.sequence(), 1);
    assert_eq!(second.interleaved_samples(), &[0.3, 0.4]);
    assert!(consumer.try_receive().is_none());
}

#[test]
fn saturation_is_counted_and_next_block_exposes_discontinuity() {
    let (mut producer, mut consumer) =
        analysis_capture_channel(1, config(AudioFormat::mono(48_000).unwrap()));

    assert_eq!(producer.try_capture(0, &[0.1, 0.2], false), CaptureResult::Captured);
    assert_eq!(producer.try_capture(2, &[0.3, 0.4], false), CaptureResult::DroppedFull);
    assert_eq!(producer.saturation().dropped_blocks, 1);
    assert_eq!(producer.saturation().dropped_frames, 2);
    assert_eq!(consumer.saturation(), producer.saturation());
    assert!(!consumer.try_receive().unwrap().discontinuity());
    assert_eq!(producer.try_capture(4, &[0.5, 0.6], false), CaptureResult::Captured);

    let after_gap = consumer.try_receive().unwrap();
    assert_eq!(after_gap.sequence(), 2);
    assert!(after_gap.discontinuity());
}

#[test]
fn capture_preserves_metadata_and_stereo_frame_boundaries() {
    let (mut producer, mut consumer) =
        analysis_capture_channel(1, config(AudioFormat::stereo(96_000).unwrap()));

    assert_eq!(producer.try_capture(42, &[0.1, -0.1, 0.2, -0.2], true), CaptureResult::Captured);
    let block = consumer.try_receive().unwrap();

    assert_eq!(block.source_id(), &SourceId::new("player"));
    assert_eq!(block.session_id(), &SessionId::new("session-1"));
    assert_eq!(block.source_kind(), SourceKind::Playback);
    assert_eq!(block.measurement_point(), MeasurementPoint::Source);
    assert_eq!(block.format(), AudioFormat::stereo(96_000).unwrap());
    assert_eq!(block.first_sample(), 42);
    assert_eq!(block.frame_count(), 2);
    assert_eq!(block.sequence(), 0);
    assert!(block.discontinuity());
    assert_eq!(block.interleaved_samples(), &[0.1, -0.1, 0.2, -0.2]);
}

#[test]
fn invalid_or_oversized_blocks_are_rejected_without_publication() {
    let (mut producer, mut consumer) =
        analysis_capture_channel(1, config(AudioFormat::stereo(48_000).unwrap()));
    let oversized = vec![0.0; MAX_ANALYSIS_CAPTURE_SAMPLES + 2];

    assert_eq!(producer.try_capture(0, &[0.1], false), CaptureResult::InvalidBlock);
    assert_eq!(producer.try_capture(0, &oversized, false), CaptureResult::InvalidBlock);
    assert_eq!(producer.try_capture(0, &[f32::NAN, 0.0], false), CaptureResult::InvalidBlock);
    assert!(consumer.try_receive().is_none());
    assert_eq!(producer.saturation().dropped_blocks, 0);
}

#[test]
fn saturation_leaves_playback_samples_unchanged() {
    let (mut producer, _consumer) =
        analysis_capture_channel(1, config(AudioFormat::mono(48_000).unwrap()));
    let playback = [0.25, 0.5, 0.75, 1.0];
    assert_eq!(producer.try_capture(0, &playback, false), CaptureResult::Captured);
    assert_eq!(producer.try_capture(4, &playback, false), CaptureResult::DroppedFull);
    assert_eq!(playback, [0.25, 0.5, 0.75, 1.0]);
}

#[test]
fn full_queue_never_waits_for_stalled_consumer() {
    let (mut producer, _consumer) =
        analysis_capture_channel(1, config(AudioFormat::mono(48_000).unwrap()));
    assert_eq!(producer.try_capture(0, &[0.0], false), CaptureResult::Captured);

    thread::scope(|scope| {
        scope
            .spawn(move || {
                for first_sample in 1..=10_000 {
                    assert_eq!(
                        producer.try_capture(first_sample, &[0.0], false),
                        CaptureResult::DroppedFull
                    );
                }
            })
            .join()
            .unwrap();
    });
}

#[test]
fn shutdown_and_disconnected_ends_are_explicit() {
    let (mut producer, mut consumer) =
        analysis_capture_channel(1, config(AudioFormat::mono(48_000).unwrap()));
    consumer.shutdown();

    assert!(producer.is_shutdown());
    assert!(consumer.is_shutdown());
    assert_eq!(producer.try_capture(0, &[0.0], false), CaptureResult::Shutdown);
    assert!(consumer.try_receive().is_none());

    let (mut producer, consumer) =
        analysis_capture_channel(1, config(AudioFormat::mono(48_000).unwrap()));
    drop(consumer);
    assert_eq!(producer.try_capture(0, &[0.0], false), CaptureResult::ConsumerGone);

    let (mut producer, mut consumer) =
        analysis_capture_channel(1, config(AudioFormat::mono(48_000).unwrap()));
    assert_eq!(producer.try_capture(0, &[0.0], false), CaptureResult::Captured);
    drop(producer);
    assert!(consumer.try_receive().is_some());
    assert!(consumer.is_closed());
}

#[test]
fn unsupported_layout_is_rejected_before_channel_creation() {
    assert!(AnalysisCaptureConfig::try_new(
        SourceId::new("surround"),
        SessionId::new("session-2"),
        SourceKind::Playback,
        MeasurementPoint::Monitor,
        48_000,
        6,
    )
    .is_err());
}

#[test]
fn session_rotation_resets_sequence_and_changes_session_identity() {
    let (mut producer, mut consumer) =
        analysis_capture_channel(2, config(AudioFormat::mono(48_000).unwrap()));

    assert_eq!(producer.try_capture(0, &[0.1], false), CaptureResult::Captured);
    producer.start_new_session();
    assert_eq!(producer.try_capture(24_000, &[0.2], false), CaptureResult::Captured);

    let first = consumer.try_receive().unwrap();
    let second = consumer.try_receive().unwrap();
    assert_eq!(first.session_id(), &SessionId::new("session-1"));
    assert_eq!(first.sequence(), 0);
    assert_eq!(second.source_id(), &SourceId::new("player"));
    assert_eq!(second.session_id(), &SessionId::new("session-1-next"));
    assert_eq!(second.sequence(), 0);
    assert_eq!(second.first_sample(), 24_000);
}

#[test]
fn source_change_after_seek_never_reuses_session_identity() {
    let (mut producer, mut consumer) =
        analysis_capture_channel(3, config(AudioFormat::mono(48_000).unwrap()));

    producer.start_new_session();
    assert_eq!(producer.try_capture(1_000, &[0.1], true), CaptureResult::Captured);
    producer.start_new_source(44_100).unwrap();
    assert_eq!(producer.try_capture(0, &[0.2], false), CaptureResult::Captured);

    let after_seek = consumer.try_receive().unwrap();
    let new_source = consumer.try_receive().unwrap();
    assert_eq!(after_seek.source_id(), &SourceId::new("player"));
    assert_eq!(after_seek.session_id(), &SessionId::new("session-1-next"));
    assert_eq!(new_source.source_id(), &SourceId::new("player-next"));
    assert_eq!(new_source.session_id(), &SessionId::new("session-1-next-next"));
    assert_eq!(new_source.format(), AudioFormat::mono(44_100).unwrap());
}
