use crate::*;
use pulseseek_domain::decoder::{DecodeError, Decoder};
use std::time::Duration;

/// A fake decoder that produces a ramp of known values.
struct RampDecoder {
    data: Vec<f32>,
    position: usize,
}

impl RampDecoder {
    fn new(len: usize) -> Self {
        let data: Vec<f32> = (0..len).map(|i| i as f32).collect();
        Self { data, position: 0 }
    }
}

impl Decoder for RampDecoder {
    fn probe(&self) -> pulseseek_domain::decoder::ProbeResult {
        pulseseek_domain::decoder::ProbeResult::Supported
    }

    fn metadata(&mut self) -> Result<pulseseek_domain::decoder::StreamMetadata, DecodeError> {
        unimplemented!("not used in tests")
    }

    fn read(&mut self, buf: &mut [f32]) -> Result<usize, DecodeError> {
        let remaining = self.data.len() - self.position;
        let to_copy = buf.len().min(remaining);
        if to_copy == 0 {
            return Ok(0);
        }
        buf[..to_copy].copy_from_slice(&self.data[self.position..self.position + to_copy]);
        self.position += to_copy;
        Ok(to_copy)
    }

    fn seek(
        &mut self,
        target: pulseseek_domain::playback::position::SeekTarget,
    ) -> Result<pulseseek_domain::playback::position::Position, DecodeError> {
        self.position = (target.position().as_millis() as usize).min(self.data.len());
        Ok(target.position())
    }
}

struct InvalidFrameDecoder;

impl Decoder for InvalidFrameDecoder {
    fn probe(&self) -> pulseseek_domain::decoder::ProbeResult {
        pulseseek_domain::decoder::ProbeResult::Supported
    }

    fn metadata(&mut self) -> Result<pulseseek_domain::decoder::StreamMetadata, DecodeError> {
        unimplemented!("not used in tests")
    }

    fn read(&mut self, buf: &mut [f32]) -> Result<usize, DecodeError> {
        Ok(buf.len() + 1)
    }

    fn seek(
        &mut self,
        _target: pulseseek_domain::playback::position::SeekTarget,
    ) -> Result<pulseseek_domain::playback::position::Position, DecodeError> {
        unimplemented!("not used in tests")
    }
}

struct RejectingSeekDecoder {
    inner: RampDecoder,
}

impl Decoder for RejectingSeekDecoder {
    fn probe(&self) -> pulseseek_domain::decoder::ProbeResult {
        self.inner.probe()
    }

    fn metadata(&mut self) -> Result<pulseseek_domain::decoder::StreamMetadata, DecodeError> {
        unimplemented!("not used in tests")
    }

    fn read(&mut self, buf: &mut [f32]) -> Result<usize, DecodeError> {
        self.inner.read(buf)
    }

    fn seek(
        &mut self,
        _target: pulseseek_domain::playback::position::SeekTarget,
    ) -> Result<pulseseek_domain::playback::position::Position, DecodeError> {
        Err(DecodeError::new(
            pulseseek_domain::error::DiagnosticContext::new(
                pulseseek_domain::error::DiagnosticCode::AudioOutput,
            ),
            std::io::Error::other("seek unsupported"),
        ))
    }
}

struct CorruptTailDecoder {
    emitted: bool,
}

impl Decoder for CorruptTailDecoder {
    fn probe(&self) -> pulseseek_domain::decoder::ProbeResult {
        pulseseek_domain::decoder::ProbeResult::Supported
    }

    fn metadata(&mut self) -> Result<pulseseek_domain::decoder::StreamMetadata, DecodeError> {
        unimplemented!("not used in tests")
    }

    fn read(&mut self, buf: &mut [f32]) -> Result<usize, DecodeError> {
        if !self.emitted {
            self.emitted = true;
            let count = 4.min(buf.len());
            buf[..count].fill(0.25);
            Ok(count)
        } else {
            Err(DecodeError::new(
                pulseseek_domain::error::DiagnosticContext::new(
                    pulseseek_domain::error::DiagnosticCode::AudioOutput,
                ),
                std::io::Error::other("corrupt tail"),
            ))
        }
    }

    fn seek(
        &mut self,
        _target: pulseseek_domain::playback::position::SeekTarget,
    ) -> Result<pulseseek_domain::playback::position::Position, DecodeError> {
        unimplemented!("not used in tests")
    }
}

#[test]
fn frames_output_in_order() {
    let decoder = Box::new(RampDecoder::new(100));
    let (mut engine, mut consumer) = PlaybackEngine::new(decoder, 256);

    assert!(engine.process_chunk().unwrap(), "should have data");

    let mut out = vec![0.0f32; 100];
    let n = consumer.consume(&mut out);
    assert_eq!(n, 100);
    for (i, &v) in out.iter().enumerate().take(100) {
        assert_eq!(v, i as f32, "mismatch at position {i}");
    }
}

#[test]
fn invalid_decoder_frame_count_returns_error() {
    let (mut engine, _) = PlaybackEngine::new(Box::new(InvalidFrameDecoder), 16);

    let error = engine.process_chunk().unwrap_err();
    assert!(error.to_string().contains("decoder returned 17 frames"));
}

#[test]
fn mono_source_is_duplicated_for_stereo_output() {
    let (mut engine, mut consumer) = PlaybackEngine::new(Box::new(RampDecoder::new(2)), 16);
    assert!(engine.process_chunk().unwrap());

    let mut output = [0.0f32; 4];
    let written = consumer.consume_channels(&mut output, 1, 2);

    assert_eq!(written, 4);
    assert_eq!(output, [0.0, 0.0, 1.0, 1.0]);
}

#[test]
fn stereo_source_is_averaged_for_mono_output() {
    let (mut engine, mut consumer) = PlaybackEngine::new(Box::new(RampDecoder::new(4)), 16);
    assert!(engine.process_chunk().unwrap());

    let mut output = [0.0f32; 2];
    let written = consumer.consume_channels(&mut output, 2, 1);

    assert_eq!(written, 2);
    assert_eq!(output, [0.5, 2.5]);
}

#[test]
fn channel_starvation_does_not_consume_partial_source_frame() {
    let (mut engine, mut consumer) = PlaybackEngine::new(Box::new(RampDecoder::new(2)), 16);
    assert!(engine.process_chunk().unwrap());

    let mut first_sample = [0.0f32; 1];
    assert_eq!(consumer.consume(&mut first_sample), 1);
    assert_eq!(consumer.available(), 1);

    let mut output = [9.0f32; 2];
    assert_eq!(consumer.consume_channels(&mut output, 2, 2), 0);
    assert_eq!(output, [0.0, 0.0]);
    assert_eq!(consumer.available(), 1);
}

#[test]
#[should_panic(expected = "playback buffer must contain at least one frame")]
fn zero_capacity_buffer_is_rejected() {
    let _ = PlaybackEngine::new(Box::new(RampDecoder::new(1)), 0);
}

#[test]
fn starvation_returns_zero_frames() {
    let decoder = Box::new(RampDecoder::new(10));
    let (mut engine, mut consumer) = PlaybackEngine::new(decoder, 256);

    // Process one chunk — should consume all 10 frames.
    assert!(engine.process_chunk().unwrap(), "should have data");
    let mut buf = vec![0.0f32; 256];
    let n = consumer.consume(&mut buf);
    assert_eq!(n, 10);

    // No more data — process_chunk returns false.
    assert!(!engine.process_chunk().unwrap(), "should be EOF");
    assert!(engine.is_finished(), "engine should be finished");
}

#[test]
fn partial_read_handled_gracefully() {
    // Decoder has 5 frames, buffer asks for 256.
    let decoder = Box::new(RampDecoder::new(5));
    let (mut engine, mut consumer) = PlaybackEngine::new(decoder, 256);

    assert!(engine.process_chunk().unwrap(), "should have data");
    assert_eq!(engine.frames_written, 5, "only 5 frames written");

    let mut out = vec![0.0f32; 256];
    let n = consumer.consume(&mut out);
    assert_eq!(n, 5, "only 5 frames consumed");
}

#[test]
fn multiple_chunks_produce_continuous_output() {
    let decoder = Box::new(RampDecoder::new(300));
    let (mut engine, mut consumer) = PlaybackEngine::new(decoder, 256);

    // First chunk: decoder fills ring buffer with 256 frames.
    assert!(engine.process_chunk().unwrap());
    assert_eq!(engine.available(), 256);

    // Consume first batch (simulates audio callback).
    let mut batch1 = vec![0.0f32; 256];
    let n1 = consumer.consume(&mut batch1);
    assert_eq!(n1, 256);

    // Second chunk: push remaining 44 frames.
    assert!(engine.process_chunk().unwrap());
    assert_eq!(engine.available(), 44);

    // Consume second batch.
    let mut batch2 = vec![0.0f32; 44];
    let n2 = consumer.consume(&mut batch2);
    assert_eq!(n2, 44);

    // Verify continuity: batch1[0..256] + batch2[0..44] == 0..300
    for (i, &value) in batch1.iter().enumerate() {
        assert_eq!(value, i as f32, "mismatch at batch1[{i}]");
    }
    for (i, &value) in batch2.iter().enumerate() {
        assert_eq!(value, (256 + i) as f32, "mismatch at batch2[{i}]");
    }
}

#[test]
fn worker_streams_fixture_to_consumer_until_eof() {
    let decoder = Box::new(RampDecoder::new(300));
    let (worker, mut consumer) = PlaybackWorker::start(decoder, 64);
    let mut output = Vec::new();
    let mut scratch = [0.0f32; 16];

    for _ in 0..100_000 {
        let count = consumer.consume(&mut scratch);
        output.extend_from_slice(&scratch[..count]);
        if worker.is_finished() && consumer.available() == 0 {
            break;
        }
        std::thread::yield_now();
    }

    worker.join().unwrap();
    assert_eq!(output, (0..300).map(|i| i as f32).collect::<Vec<_>>());
}

#[test]
fn volume_at_unity_preserves_samples() {
    assert_eq!(apply_volume(0.25, 1.0), 0.25);
    assert_eq!(apply_volume(-0.75, 1.0), -0.75);
}

#[test]
fn volume_attenuates_samples() {
    assert_eq!(apply_volume(0.8, 0.5), 0.4);
    assert_eq!(apply_volume(-0.8, 0.5), -0.4);
}

#[test]
fn muted_volume_outputs_silence() {
    assert_eq!(apply_volume(0.8, 0.0), 0.0);
    assert_eq!(apply_volume(-0.8, 0.0), 0.0);
}

#[test]
fn over_unity_volume_hard_clips_samples() {
    assert_eq!(apply_volume(0.75, 2.0), 1.0);
    assert_eq!(apply_volume(-0.75, 2.0), -1.0);
}

#[test]
fn callback_render_applies_volume_to_mapped_samples() {
    let (mut engine, mut consumer) = PlaybackEngine::new(Box::new(RampDecoder::new(2)), 16);
    assert!(engine.process_chunk().unwrap());

    let mut output = [0.0f32; 4];
    let written = consumer.consume_channels_with_volume(&mut output, 1, 2, 0.5);

    assert_eq!(written, 4);
    assert_eq!(output, [0.0, 0.0, 0.5, 0.5]);
}

#[test]
fn pause_preserves_buffer_position_until_resume() {
    let (mut engine, mut consumer) = PlaybackEngine::new(Box::new(RampDecoder::new(4)), 16);
    assert!(engine.process_chunk().unwrap());
    let control = consumer.control();

    control.pause();
    let mut paused_output = [9.0f32; 2];
    assert_eq!(consumer.consume(&mut paused_output), 0);
    assert_eq!(paused_output, [0.0, 0.0]);
    assert!(control.is_paused());
    assert_eq!(consumer.available(), 4);

    control.resume();
    let mut resumed_output = [0.0f32; 2];
    assert_eq!(consumer.consume(&mut resumed_output), 2);
    assert_eq!(resumed_output, [0.0, 1.0]);
    assert!(!control.is_paused());
}

#[test]
fn repeated_pause_is_idempotent() {
    let (mut engine, consumer) = PlaybackEngine::new(Box::new(RampDecoder::new(1)), 16);
    assert!(engine.process_chunk().unwrap());
    let control = consumer.control();

    control.pause();
    control.pause();
    assert!(control.is_paused());
    control.resume();
    control.resume();
    assert!(!control.is_paused());
}

#[test]
fn end_while_paused_keeps_buffered_frames_for_resume() {
    let (worker, mut consumer) = PlaybackWorker::start(Box::new(RampDecoder::new(4)), 16);
    let control = consumer.control();
    control.pause();
    worker.wait().unwrap();

    let mut paused_output = [9.0f32; 4];
    assert_eq!(consumer.consume(&mut paused_output), 0);
    assert_eq!(paused_output, [0.0; 4]);
    assert_eq!(consumer.available(), 4);

    control.resume();
    let mut resumed_output = [0.0f32; 4];
    assert_eq!(consumer.consume(&mut resumed_output), 4);
    assert_eq!(resumed_output, [0.0, 1.0, 2.0, 3.0]);
}

#[test]
fn stop_discards_visible_position_and_silences_consumer() {
    let (mut engine, mut consumer) = PlaybackEngine::new(Box::new(RampDecoder::new(4)), 16);
    assert!(engine.process_chunk().unwrap());
    let control = consumer.control();

    control.stop();
    let mut output = [9.0f32; 4];
    assert_eq!(consumer.consume(&mut output), 0);
    assert_eq!(output, [0.0; 4]);
    assert_eq!(consumer.available(), 0);
    assert!(control.is_stopped());

    control.stop();
    assert!(control.is_stopped());
}

#[test]
fn stop_terminates_decoder_worker() {
    let (worker, consumer) = PlaybackWorker::start(Box::new(RampDecoder::new(1_000_000)), 16);
    let control = consumer.control();
    control.stop();

    worker.wait().unwrap();
    assert!(control.is_stopped());
}

#[test]
fn seek_while_playing_discards_stale_frames() {
    let (worker, mut consumer) = PlaybackWorker::start(Box::new(RampDecoder::new(1_000)), 16);
    while consumer.available() < 16 {
        std::thread::yield_now();
    }

    worker.seek(seek_target(50)).unwrap();

    let mut output = [0.0f32; 4];
    for _ in 0..10_000 {
        if consumer.consume(&mut output) == 4 {
            break;
        }
        std::thread::yield_now();
    }
    assert_eq!(output, [50.0, 51.0, 52.0, 53.0]);
    worker.join().unwrap();
}

#[test]
fn seek_while_paused_preserves_pause_and_resumes_at_target() {
    let (worker, mut consumer) = PlaybackWorker::start(Box::new(RampDecoder::new(1_000)), 16);
    let control = consumer.control();
    control.pause();

    worker.seek(seek_target(75)).unwrap();

    let mut paused_output = [9.0f32; 2];
    assert_eq!(consumer.consume(&mut paused_output), 0);
    assert_eq!(paused_output, [0.0, 0.0]);

    control.resume();
    let mut output = [0.0f32; 4];
    for _ in 0..10_000 {
        if consumer.consume(&mut output) == 4 {
            break;
        }
        std::thread::yield_now();
    }
    assert_eq!(output, [75.0, 76.0, 77.0, 78.0]);
    control.stop();
    worker.join().unwrap();
}

#[test]
fn repeated_seek_ends_at_latest_target() {
    let (worker, mut consumer) = PlaybackWorker::start(Box::new(RampDecoder::new(1_000)), 16);
    while consumer.available() < 16 {
        std::thread::yield_now();
    }

    worker.seek(seek_target(20)).unwrap();
    worker.seek(seek_target(90)).unwrap();

    let mut output = [0.0f32; 2];
    for _ in 0..10_000 {
        if consumer.consume(&mut output) == 2 {
            break;
        }
        std::thread::yield_now();
    }
    assert_eq!(output, [90.0, 91.0]);
    control_stop_and_join(worker, consumer);
}

#[test]
fn unsupported_seek_returns_decoder_error() {
    let (worker, consumer) = PlaybackWorker::start(
        Box::new(RejectingSeekDecoder { inner: RampDecoder::new(1_000) }),
        16,
    );
    while consumer.available() < 16 {
        std::thread::yield_now();
    }

    assert!(worker.seek(seek_target(20)).is_err());
    control_stop_and_join(worker, consumer);
}

#[test]
fn seek_after_decoder_eof_reopens_playback_position() {
    let (worker, mut consumer) = PlaybackWorker::start(Box::new(RampDecoder::new(4)), 16);
    while !worker.is_finished() {
        std::thread::yield_now();
    }

    worker.seek(seek_target(2)).unwrap();
    let mut output = [0.0f32; 2];
    for _ in 0..10_000 {
        if consumer.consume(&mut output) == 2 {
            break;
        }
        std::thread::yield_now();
    }
    assert_eq!(output, [2.0, 3.0]);
    control_stop_and_join(worker, consumer);
}

fn control_stop_and_join(worker: PlaybackWorker, consumer: PlaybackConsumer) {
    consumer.control().stop();
    let _ = worker.join();
}

fn seek_target(milliseconds: u64) -> pulseseek_domain::playback::position::SeekTarget {
    pulseseek_domain::playback::position::Duration::Unknown
        .seek_to(pulseseek_domain::playback::position::Position::from_millis(milliseconds))
        .unwrap()
}

#[test]
fn equal_sample_rates_bypass_resampling_without_losing_samples() {
    let (worker, mut consumer) =
        PlaybackWorker::start_resampled(Box::new(RampDecoder::new(64)), 128, 1, 48_000, 48_000)
            .unwrap();
    let output = collect_worker_output(worker, &mut consumer);

    assert_eq!(output, (0..64).map(|value| value as f32).collect::<Vec<_>>());
}

#[test]
fn resampling_44100_to_48000_preserves_duration_ratio() {
    let (worker, mut consumer) =
        PlaybackWorker::start_resampled(Box::new(RampDecoder::new(441)), 256, 1, 44_100, 48_000)
            .unwrap();
    let output = collect_worker_output(worker, &mut consumer);

    assert_eq!(output.len(), 480);
    assert!(output.iter().any(|sample| *sample > 0.0));
}

#[test]
fn resampling_48000_to_44100_preserves_duration_ratio() {
    let (worker, mut consumer) =
        PlaybackWorker::start_resampled(Box::new(RampDecoder::new(480)), 256, 1, 48_000, 44_100)
            .unwrap();
    let output = collect_worker_output(worker, &mut consumer);

    assert_eq!(output.len(), 441);
    assert!(output.iter().any(|sample| *sample > 0.0));
}

#[test]
fn resampling_stereo_keeps_interleaved_channel_sample_count() {
    let (worker, mut consumer) =
        PlaybackWorker::start_resampled(Box::new(RampDecoder::new(882)), 256, 2, 44_100, 48_000)
            .unwrap();
    let output = collect_worker_output(worker, &mut consumer);

    assert_eq!(output.len(), 960);
}

#[test]
fn resampling_rejects_invalid_configuration() {
    assert!(PlaybackWorker::start_resampled(Box::new(RampDecoder::new(4)), 16, 0, 44_100, 48_000,)
        .is_err());
    assert!(
        PlaybackWorker::start_resampled(Box::new(RampDecoder::new(4)), 16, 1, 0, 48_000,).is_err()
    );
}

#[test]
fn one_shot_completion_waits_for_buffer_drain_and_emits_once() {
    let (worker, mut consumer) = PlaybackWorker::start(Box::new(RampDecoder::new(4)), 16);
    while !worker.is_finished() {
        std::thread::yield_now();
    }
    assert!(worker.poll_event().is_none());

    let mut output = [0.0f32; 4];
    assert_eq!(consumer.consume(&mut output), 4);

    let event = wait_for_event(&worker);
    assert!(matches!(event, PlaybackEvent::Completed));
    assert!(worker.poll_event().is_none());
    worker.join().unwrap();
}

#[test]
fn empty_decoder_emits_one_completion_event() {
    let (worker, consumer) = PlaybackWorker::start(Box::new(RampDecoder::new(0)), 16);
    while !worker.is_finished() {
        std::thread::yield_now();
    }

    let event = wait_for_event(&worker);
    assert!(matches!(event, PlaybackEvent::Completed));
    assert!(worker.poll_event().is_none());
    control_stop_and_join(worker, consumer);
}

#[test]
fn corrupt_tail_emits_failure_without_completion() {
    let (worker, consumer) =
        PlaybackWorker::start(Box::new(CorruptTailDecoder { emitted: false }), 16);
    let event = wait_for_event(&worker);

    assert!(matches!(event, PlaybackEvent::Failed));
    assert!(worker.poll_event().is_none());
    control_stop_and_join(worker, consumer);
}

#[test]
fn resampled_eof_completes_after_converted_tail_drains() {
    let (worker, mut consumer) =
        PlaybackWorker::start_resampled(Box::new(RampDecoder::new(441)), 256, 1, 44_100, 48_000)
            .unwrap();
    let mut output = [0.0f32; 64];
    for _ in 0..100_000 {
        let _ = consumer.consume(&mut output);
        if worker.is_finished() && consumer.available() == 0 {
            break;
        }
        std::thread::yield_now();
    }

    assert!(matches!(wait_for_event(&worker), PlaybackEvent::Completed));
    let _ = worker.join();
}

#[test]
fn loop_current_replays_multiple_cycles_without_stale_frames() {
    let (worker, mut consumer) = PlaybackWorker::start_with_mode(
        Box::new(RampDecoder::new(4)),
        4,
        pulseseek_domain::playback::mode::PlaybackMode::LoopCurrent,
    );
    let mut output = Vec::new();
    let mut scratch = [0.0f32; 2];
    for _ in 0..100_000 {
        let count = consumer.consume(&mut scratch);
        output.extend_from_slice(&scratch[..count]);
        if output.len() >= 12 {
            break;
        }
        std::thread::yield_now();
    }

    assert_eq!(output, vec![0.0, 1.0, 2.0, 3.0, 0.0, 1.0, 2.0, 3.0, 0.0, 1.0, 2.0, 3.0]);
    assert!(worker.poll_event().is_none());
    control_stop_and_join(worker, consumer);
}

#[test]
fn short_loop_replays_from_prebuffer_without_decoder_seek() {
    let (worker, mut consumer) = PlaybackWorker::start_with_mode(
        Box::new(RejectingSeekDecoder { inner: RampDecoder::new(1) }),
        1,
        pulseseek_domain::playback::mode::PlaybackMode::LoopCurrent,
    );
    let mut output = [0.0f32; 8];

    let mut produced = 0;
    for _ in 0..100_000 {
        let count = consumer.consume(&mut output[produced..]);
        produced += count;
        assert!(worker.poll_event().is_none(), "short loop terminated early");
        if produced == output.len() {
            break;
        }
        std::thread::yield_now();
    }

    assert_eq!(produced, output.len());
    assert_eq!(output, [0.0; 8]);
    assert!(worker.poll_event().is_none());
    control_stop_and_join(worker, consumer);
}

#[test]
fn short_loop_keeps_four_sample_cycle_contiguous() {
    let (worker, mut consumer) = PlaybackWorker::start_with_mode(
        Box::new(RejectingSeekDecoder { inner: RampDecoder::new(4) }),
        4,
        pulseseek_domain::playback::mode::PlaybackMode::LoopCurrent,
    );
    let mut output = [0.0f32; 16];
    let mut produced = 0;

    for _ in 0..100_000 {
        let count = consumer.consume(&mut output[produced..]);
        produced += count;
        assert!(worker.poll_event().is_none(), "short loop terminated early");
        if produced == output.len() {
            break;
        }
        std::thread::yield_now();
    }

    assert_eq!(produced, output.len());
    assert_eq!(
        output,
        [0.0, 1.0, 2.0, 3.0, 0.0, 1.0, 2.0, 3.0, 0.0, 1.0, 2.0, 3.0, 0.0, 1.0, 2.0, 3.0]
    );
    control_stop_and_join(worker, consumer);
}

#[test]
fn loop_longer_than_buffer_keeps_seek_fallback() {
    let (worker, mut consumer) = PlaybackWorker::start_with_mode(
        Box::new(RampDecoder::new(8)),
        4,
        pulseseek_domain::playback::mode::PlaybackMode::LoopCurrent,
    );
    let mut output = [0.0f32; 16];
    let mut produced = 0;

    for _ in 0..100_000 {
        let count = consumer.consume(&mut output[produced..]);
        produced += count;
        assert!(worker.poll_event().is_none(), "long loop terminated early");
        if produced == output.len() {
            break;
        }
        std::thread::yield_now();
    }

    assert_eq!(produced, output.len());
    assert_eq!(
        output,
        [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]
    );
    control_stop_and_join(worker, consumer);
}

#[test]
fn changing_loop_to_one_shot_near_boundary_completes_once() {
    let (worker, mut consumer) = PlaybackWorker::start_with_mode(
        Box::new(RampDecoder::new(4)),
        4,
        pulseseek_domain::playback::mode::PlaybackMode::LoopCurrent,
    );
    let mut first_cycle = [0.0f32; 4];
    while consumer.consume(&mut first_cycle) != 4 {
        std::thread::yield_now();
    }

    worker.set_mode(pulseseek_domain::playback::mode::PlaybackMode::OneShot).unwrap();
    let mut scratch = [0.0f32; 2];
    for _ in 0..100_000 {
        let _ = consumer.consume(&mut scratch);
        if let Some(PlaybackEvent::Completed) = worker.poll_event() {
            break;
        }
        std::thread::yield_now();
    }
    assert!(worker.poll_event().is_none());
    let _ = worker.join();
}

#[test]
fn stop_during_loop_prevents_restart_and_completion() {
    let (worker, consumer) = PlaybackWorker::start_with_mode(
        Box::new(RampDecoder::new(4)),
        4,
        pulseseek_domain::playback::mode::PlaybackMode::LoopCurrent,
    );
    consumer.control().stop();

    assert!(worker.poll_event().is_none());
    worker.join().unwrap();
}

#[test]
fn empty_loop_fails_instead_of_spinning_forever() {
    let (worker, consumer) = PlaybackWorker::start_with_mode(
        Box::new(RampDecoder::new(0)),
        4,
        pulseseek_domain::playback::mode::PlaybackMode::LoopCurrent,
    );

    assert!(matches!(wait_for_event(&worker), PlaybackEvent::Failed));
    control_stop_and_join(worker, consumer);
}

#[test]
fn stop_race_does_not_emit_completion() {
    let (worker, consumer) = PlaybackWorker::start(Box::new(RampDecoder::new(1_000_000)), 16);
    consumer.control().stop();

    assert!(worker.poll_event().is_none());
    worker.join().unwrap();
}

fn wait_for_event(worker: &PlaybackWorker) -> PlaybackEvent {
    for _ in 0..100_000 {
        if let Some(event) = worker.poll_event() {
            return event;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("playback event not emitted");
}

fn collect_worker_output(worker: PlaybackWorker, consumer: &mut PlaybackConsumer) -> Vec<f32> {
    let mut output = Vec::new();
    let mut scratch = [0.0f32; 64];
    for _ in 0..100_000 {
        let count = consumer.consume(&mut scratch);
        output.extend_from_slice(&scratch[..count]);
        if worker.is_finished() && consumer.available() == 0 {
            break;
        }
        std::thread::yield_now();
    }
    worker.join().unwrap();
    output
}
