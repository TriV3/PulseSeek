use crate::*;
use pulseseek_domain::decoder::{DecodeError, Decoder};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;

/// A fake decoder that produces a ramp of known values.
struct RampDecoder {
    data: Vec<f32>,
    position: usize,
}

/// Stereo fake whose sample values identify their source frame.
struct StereoRampDecoder {
    data: Vec<f32>,
    position: usize,
}

impl StereoRampDecoder {
    fn new(frames: usize) -> Self {
        let data = (0..frames).flat_map(|frame| [frame as f32, frame as f32]).collect();
        Self { data, position: 0 }
    }
}

impl Decoder for StereoRampDecoder {
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
        self.position = (target.position().as_millis() as usize * 2).min(self.data.len());
        Ok(target.position())
    }
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

struct RejectSecondSeekDecoder {
    inner: RampDecoder,
    seeks: usize,
}

impl Decoder for RejectSecondSeekDecoder {
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
        target: pulseseek_domain::playback::position::SeekTarget,
    ) -> Result<pulseseek_domain::playback::position::Position, DecodeError> {
        self.seeks += 1;
        if self.seeks == 2 {
            return Err(DecodeError::new(
                pulseseek_domain::error::DiagnosticContext::new(
                    pulseseek_domain::error::DiagnosticCode::AudioOutput,
                ),
                std::io::Error::other("second seek rejected"),
            ));
        }
        self.inner.seek(target)
    }
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

struct RecordingSeekDecoder {
    inner: RampDecoder,
    seeks: Arc<AtomicU64>,
}

impl Decoder for RecordingSeekDecoder {
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
        target: pulseseek_domain::playback::position::SeekTarget,
    ) -> Result<pulseseek_domain::playback::position::Position, DecodeError> {
        self.seeks.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.inner.seek(target)
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
fn prepared_track_appends_without_silence_at_boundary() {
    let (mut engine, mut consumer) = PlaybackEngine::new(Box::new(RampDecoder::new(4)), 8);
    engine.prepare_next(
        Box::new(RampDecoder::new(4)),
        None,
        1_000,
        "next.wav".to_string(),
        Some(4),
    );
    engine.prime_prepared().expect("prime next track");
    assert!(engine.process_chunk().unwrap());
    assert!(!engine.process_chunk().unwrap());
    assert!(engine.append_prepared());

    let mut output = [0.0f32; 8];
    assert_eq!(consumer.consume(&mut output), 8);
    assert_eq!(output, [0.0, 1.0, 2.0, 3.0, 0.0, 1.0, 2.0, 3.0]);
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
    assert_eq!(consumer.control().position_frames(), 2);
}

#[test]
fn playback_position_counts_frames_not_interleaved_samples() {
    let (mut engine, mut consumer) = PlaybackEngine::new(Box::new(RampDecoder::new(8)), 16);
    assert!(engine.process_chunk().unwrap());

    let control = consumer.control();
    let mut output = [0.0f32; 4];
    assert_eq!(consumer.consume_channels(&mut output, 2, 2), 4);

    assert_eq!(control.position_frames(), 2);
    control.set_position_frames(48_000);
    assert_eq!(control.position_frames(), 48_000);
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
        output.fill(0.0);
        if consumer.consume(&mut output) == 4 && output == [50.0, 51.0, 52.0, 53.0] {
            break;
        }
        std::thread::yield_now();
    }
    assert_eq!(output, [50.0, 51.0, 52.0, 53.0]);
    worker.join().unwrap();
}

#[test]
fn channel_output_smooths_the_discontinuity_after_seek() {
    let (worker, mut consumer) = PlaybackWorker::start(Box::new(RampDecoder::new(1_000)), 32);
    while consumer.available() < 32 {
        std::thread::yield_now();
    }

    let mut before = [0.0f32; 4];
    assert_eq!(consumer.consume_channels(&mut before, 1, 1), 4);
    assert_eq!(before, [0.0, 1.0, 2.0, 3.0]);

    let control = consumer.control();
    let seek_thread = std::thread::spawn(move || {
        worker.seek(seek_target(50)).unwrap();
        worker
    });
    while !control.seeking.load(std::sync::atomic::Ordering::Acquire) {
        std::thread::yield_now();
    }
    let mut fade_out = [0.0f32; 512];
    assert_eq!(consumer.consume_channels(&mut fade_out, 1, 1), 0);
    assert!(fade_out[0] > 2.9 && fade_out[0] < 3.0);
    assert!(fade_out[127] > 0.0, "fade must last longer than the previous 128 frames");
    assert_eq!(fade_out[511], 0.0);
    assert!(fade_out.windows(2).all(|samples| samples[1] <= samples[0]));
    let worker = seek_thread.join().unwrap();

    let mut after = [0.0f32; 4];
    for _ in 0..10_000 {
        if consumer.consume_channels(&mut after, 1, 1) == 4 {
            break;
        }
        std::thread::yield_now();
    }

    assert!(after[0] > 0.0, "ramp should move away from silence");
    assert!(after[0] < 1.0, "first post-seek sample must fade in from zero");
    assert!(after.windows(2).all(|samples| samples[1] > samples[0]));
    control_stop_and_join(worker, consumer);
}

#[test]
fn seek_timeout_crossfades_from_the_last_output_sample() {
    let (mut engine, mut consumer) = PlaybackEngine::new(Box::new(RampDecoder::new(32)), 8);
    assert!(engine.process_chunk().unwrap());

    let mut before = [0.0f32; 4];
    assert_eq!(consumer.consume_channels(&mut before, 1, 1), 4);
    assert_eq!(before, [0.0, 1.0, 2.0, 3.0]);

    // Simulate a hardware callback interval longer than the worker wait: the
    // seek completes before render_seek_fade gets a chance to run.
    engine.control.begin_seek();
    engine.control.complete_user_seek();
    assert!(engine.process_chunk().unwrap());

    let mut after = [0.0f32; 1];
    assert_eq!(consumer.consume_channels(&mut after, 1, 1), 0);
    assert!(
        after[0] >= 2.99,
        "the first resumed sample must continue from the previous output, got {}",
        after[0]
    );
    assert!(after[0] < 3.01, "the crossfade must start gradually, got {}", after[0]);
}

#[test]
fn seek_generation_never_scans_a_large_stale_buffer_in_one_callback() {
    let (mut engine, mut consumer) =
        PlaybackEngine::new(Box::new(RampDecoder::new(262_144)), 131_072);
    assert!(engine.process_chunk().unwrap());

    let mut before = [0.0f32; 1];
    assert_eq!(consumer.consume_channels(&mut before, 1, 1), 1);

    // Simulate completion before the callback could perform the fade. The
    // ring now contains almost 131k stale samples, matching production.
    engine.control.begin_seek();
    engine.control.complete_user_seek();
    assert!(engine.process_chunk().unwrap());

    let mut resumed = [0.0f32; 1];
    assert_eq!(consumer.consume_channels(&mut resumed, 1, 1), 0);
    assert_eq!(
        consumer.available(),
        0,
        "the callback must invalidate the stale ring in constant time instead of scanning it"
    );
}

#[test]
fn moving_loop_marker_discards_large_stale_buffer_in_one_callback() {
    let (mut engine, mut consumer) =
        PlaybackEngine::new(Box::new(RampDecoder::new(262_144)), 131_072);
    assert!(engine.process_chunk().unwrap());

    let mut before = [0.0f32; 1];
    assert_eq!(consumer.consume_channels(&mut before, 1, 1), 1);

    engine.set_loop_region(loop_region(50, 100)).unwrap();

    let mut resumed = [0.0f32; 1];
    assert_eq!(consumer.consume_channels(&mut resumed, 1, 1), 0);
    assert_eq!(
        consumer.available(),
        0,
        "moving A or B must invalidate the stale ring in constant time"
    );
}

#[test]
fn failed_loop_marker_move_preserves_existing_region() {
    let (mut engine, _) = PlaybackEngine::new(
        Box::new(RejectSecondSeekDecoder { inner: RampDecoder::new(1_000), seeks: 0 }),
        16,
    );
    let original = loop_region(10, 20);
    engine.set_loop_region(original).unwrap();

    let error =
        engine.set_loop_region(loop_region(30, 40)).expect_err("second positioning seek must fail");

    assert!(matches!(error.kind, PlaybackErrorKind::Decode(_)));
    assert_eq!(engine.loop_region, Some(original));
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
    let mut collected = Vec::new();
    for _ in 0..10_000 {
        let count = consumer.consume(&mut output);
        if count == 4 {
            collected.extend_from_slice(&output);
        }
        if collected.windows(4).any(|window| window == [75.0, 76.0, 77.0, 78.0]) {
            break;
        }
        std::thread::yield_now();
    }
    assert!(
        collected.windows(4).any(|window| window == [75.0, 76.0, 77.0, 78.0]),
        "seek target must become audible after resume"
    );
    control.stop();
    worker.join().unwrap();
}

#[test]
fn repeated_seek_ends_at_latest_target() {
    let (worker, mut consumer) = PlaybackWorker::start(Box::new(RampDecoder::new(1_000)), 16);
    let control = consumer.control();
    control.pause();
    while consumer.available() < 16 {
        std::thread::yield_now();
    }

    worker.seek(seek_target(20)).unwrap();
    worker.seek(seek_target(90)).unwrap();
    control.resume();

    let mut output = [0.0f32; 2];
    let mut collected = Vec::new();
    for _ in 0..10_000 {
        output.fill(0.0);
        if consumer.consume(&mut output) == 2 {
            collected.extend_from_slice(&output);
        }
        if collected.windows(2).any(|window| window == [90.0, 91.0]) {
            break;
        }
        std::thread::yield_now();
    }
    assert!(
        collected.windows(2).any(|window| window == [90.0, 91.0]),
        "latest seek target must become audible"
    );
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
    output.truncate(12);

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
        let count = consumer.consume_channels(&mut output[produced..], 1, 1);
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
        let count = consumer.consume_channels(&mut output[produced..], 1, 1);
        produced += count;
        assert!(worker.poll_event().is_none(), "short loop terminated early");
        if produced == output.len() {
            break;
        }
        std::thread::yield_now();
    }

    assert_eq!(produced, output.len());
    assert_eq!(consumer.control().position_frames(), 4);
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
    let control = consumer.control();
    let mut observed_restart = false;

    for _ in 0..100_000 {
        let count = consumer.consume_channels(&mut output[produced..], 1, 1);
        produced += count;
        assert!(worker.poll_event().is_none(), "long loop terminated early");
        if produced == 8 && !observed_restart {
            while consumer.available() == 0 {
                std::thread::yield_now();
            }
            assert_eq!(control.position_frames(), 0);
            observed_restart = true;
        }
        if produced == output.len() {
            break;
        }
        std::thread::yield_now();
    }

    assert_eq!(produced, output.len());
    assert!(observed_restart);
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

// ── A–B repeat (PR-088) ───────────────────────────────────────────────

fn loop_region(start_ms: u64, end_ms: u64) -> pulseseek_domain::playback::loop_region::LoopRegion {
    pulseseek_domain::playback::loop_region::LoopRegion::new(
        pulseseek_domain::playback::position::Position::from_millis(start_ms),
        pulseseek_domain::playback::position::Position::from_millis(end_ms),
        pulseseek_domain::playback::position::Duration::from_millis(1_000_000),
    )
    .expect("valid loop region")
}

fn finish_loop_region_transition(consumer: &mut PlaybackConsumer, channels: usize) {
    let mut transition = vec![0.0f32; channels];
    assert_eq!(consumer.consume_channels(&mut transition, channels, channels), 0);
    consumer.seek_ramp_frame = crate::control::SEEK_RAMP_FRAMES;
}

#[test]
fn ab_repeat_wraps_at_region_end_without_advancing() {
    let (worker, mut consumer) = PlaybackWorker::start_with_mode(
        Box::new(RampDecoder::new(1_000)),
        4,
        pulseseek_domain::playback::mode::PlaybackMode::OneShot,
    );
    worker.set_loop_region(Some(loop_region(2, 6))).unwrap();
    finish_loop_region_transition(&mut consumer, 1);

    let mut output = [0.0f32; 16];
    let mut produced = 0;
    for _ in 0..100_000 {
        let count = consumer.consume_channels(&mut output[produced..], 1, 1);
        produced += count;
        if produced == output.len() {
            break;
        }
        std::thread::yield_now();
    }

    // Region [2, 6) is half-open: samples 2, 3, 4, 5 repeat; 6 is excluded
    // and the region never advances past B.
    assert_eq!(produced, output.len());
    assert_eq!(
        output,
        [2.0, 3.0, 4.0, 5.0, 2.0, 3.0, 4.0, 5.0, 2.0, 3.0, 4.0, 5.0, 2.0, 3.0, 4.0, 5.0]
    );
    // Each cycle resets the playback clock to the region start (A = 2ms), so
    // after four cycles the clock sits at B = 6ms — the absolute position
    // matches the audio, which loops 2..6.
    assert_eq!(consumer.control().position_frames(), 6);
    assert!(worker.poll_event().is_none(), "A–B repeat must never complete or fail");
    control_stop_and_join(worker, consumer);
}

#[test]
fn ab_repeat_stereo_wraps_after_region_frames_not_interleaved_samples() {
    let (worker, mut consumer) = PlaybackWorker::start_resampled_with_mode(
        Box::new(StereoRampDecoder::new(1_000)),
        16,
        2,
        1_000,
        1_000,
        pulseseek_domain::playback::mode::PlaybackMode::OneShot,
    )
    .unwrap();
    worker.set_loop_region(Some(loop_region(2, 6))).unwrap();
    finish_loop_region_transition(&mut consumer, 2);

    let mut output = [0.0f32; 16];
    let mut produced = 0;
    for _ in 0..100_000 {
        let count = consumer.consume_channels(&mut output[produced..], 2, 2);
        produced += count;
        if produced == output.len() {
            break;
        }
        std::thread::yield_now();
    }

    assert_eq!(produced, output.len());
    assert_eq!(
        output,
        [2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 5.0, 5.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 5.0, 5.0]
    );
    control_stop_and_join(worker, consumer);
}

#[test]
fn ab_repeat_short_region_replays_without_decoder_seek() {
    let seeks = Arc::new(AtomicU64::new(0));
    let (worker, mut consumer) = PlaybackWorker::start_with_mode(
        Box::new(RecordingSeekDecoder {
            inner: RampDecoder::new(1_000),
            seeks: Arc::clone(&seeks),
        }),
        8,
        pulseseek_domain::playback::mode::PlaybackMode::OneShot,
    );
    worker.set_loop_region(Some(loop_region(0, 4))).unwrap();
    let seeks_before = seeks.load(std::sync::atomic::Ordering::Relaxed);

    let mut output = [0.0f32; 16];
    let mut produced = 0;
    for _ in 0..100_000 {
        let count = consumer.consume_channels(&mut output[produced..], 1, 1);
        produced += count;
        assert!(worker.poll_event().is_none(), "A–B repeat terminated early");
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
    // Short regions replay from the prebuffer: wrapping never seeks the
    // decoder. At most the initial positioning seek may have run when the
    // worker produced frames before the region command arrived.
    assert_eq!(
        seeks.load(std::sync::atomic::Ordering::Relaxed),
        seeks_before,
        "wrapping a short region must not seek the decoder"
    );
    assert!(worker.poll_event().is_none());
    control_stop_and_join(worker, consumer);
}

#[test]
fn ab_repeat_seek_into_region_keeps_looping() {
    let (worker, mut consumer) = PlaybackWorker::start_with_mode(
        Box::new(RampDecoder::new(1_000)),
        8,
        pulseseek_domain::playback::mode::PlaybackMode::OneShot,
    );
    worker.set_loop_region(Some(loop_region(10, 20))).unwrap();

    let mut first = [0.0f32; 8];
    while consumer.consume(&mut first) != 8 {
        std::thread::yield_now();
    }

    worker.seek(seek_target(12)).unwrap();

    let mut output = [0.0f32; 16];
    let mut produced = 0;
    for _ in 0..100_000 {
        let count = consumer.consume(&mut output[produced..]);
        produced += count;
        assert!(worker.poll_event().is_none(), "A–B repeat terminated after seek");
        if produced == output.len() {
            break;
        }
        std::thread::yield_now();
    }

    // From 12 the region plays to its end (19), then wraps back to 10.
    assert_eq!(produced, output.len());
    assert_eq!(&output[..8], &[12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0]);
    assert_eq!(&output[8..16], &[10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0]);
    control_stop_and_join(worker, consumer);
}

#[test]
fn ab_repeat_seek_before_region_disables_loop() {
    let (worker, mut consumer) = PlaybackWorker::start_with_mode(
        Box::new(RampDecoder::new(1_000)),
        8,
        pulseseek_domain::playback::mode::PlaybackMode::OneShot,
    );
    worker.set_loop_region(Some(loop_region(10, 20))).unwrap();

    let mut first = [0.0f32; 8];
    while consumer.consume(&mut first) != 8 {
        std::thread::yield_now();
    }

    worker.seek(seek_target(5)).unwrap();

    let mut output = [0.0f32; 8];
    let mut produced = 0;
    for _ in 0..100_000 {
        let count = consumer.consume(&mut output[produced..]);
        produced += count;
        if produced == output.len() {
            break;
        }
        std::thread::yield_now();
    }

    // Seeking before A disables A–B repeat: playback continues from 5
    // without ever wrapping back to 10.
    assert_eq!(produced, output.len());
    assert_eq!(output, [5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0]);
    assert!(worker.poll_event().is_none(), "no terminal event while playback continues");
    control_stop_and_join(worker, consumer);
}

#[test]
fn ab_repeat_seek_at_region_end_disables_loop_and_completes() {
    let (worker, mut consumer) = PlaybackWorker::start_with_mode(
        Box::new(RampDecoder::new(1_000)),
        8,
        pulseseek_domain::playback::mode::PlaybackMode::OneShot,
    );
    worker.set_loop_region(Some(loop_region(10, 20))).unwrap();

    let mut first = [0.0f32; 8];
    while consumer.consume(&mut first) != 8 {
        std::thread::yield_now();
    }

    // B is exclusive, so 20 is outside the region: the loop is disabled.
    worker.seek(seek_target(20)).unwrap();

    let mut output = [0.0f32; 8];
    let mut produced = 0;
    for _ in 0..100_000 {
        let count = consumer.consume(&mut output[produced..]);
        produced += count;
        if produced == output.len() {
            break;
        }
        std::thread::yield_now();
    }
    assert_eq!(produced, output.len());
    assert_eq!(output, [20.0, 21.0, 22.0, 23.0, 24.0, 25.0, 26.0, 27.0]);

    // Consume the remainder while watching for the terminal event. One-shot
    // mode: playback completes at the end of the file without wrapping,
    // proving the file was never advanced past its region.
    let mut scratch = [0.0f32; 64];
    let mut terminal = None;
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while std::time::Instant::now() < deadline {
        let _ = consumer.consume(&mut scratch);
        if let Some(event) = worker.poll_event() {
            terminal = Some(event);
            break;
        }
        std::thread::yield_now();
    }
    assert!(matches!(terminal, Some(PlaybackEvent::Completed)));
    let _ = worker.join();
}

#[test]
fn clear_loop_region_stops_wrapping_and_follows_mode() {
    let (worker, mut consumer) = PlaybackWorker::start_with_mode(
        Box::new(RampDecoder::new(1_000)),
        8,
        pulseseek_domain::playback::mode::PlaybackMode::OneShot,
    );
    let control = consumer.control();
    control.pause();
    worker.set_loop_region(Some(loop_region(2, 6))).unwrap();

    let mut first = [0.0f32; 8];
    // Acknowledge the worker's discard request while paused, then start from
    // the newly committed region instead of racing pre-command samples.
    let _ = consumer.consume(&mut first);
    control.resume();
    let mut saw_region_start = false;
    for _ in 0..10_000 {
        if consumer.consume(&mut first) == 8
            && first.windows(4).any(|window| window == [2.0, 3.0, 4.0, 5.0])
        {
            saw_region_start = true;
            break;
        }
        std::thread::yield_now();
    }
    assert!(saw_region_start, "region loops before the clear");

    worker.clear_loop_region().unwrap();

    // Playback continues through B (6) and beyond without wrapping, then
    // completes at the end of the file (OneShot mode).
    let mut output = Vec::new();
    let mut scratch = [0.0f32; 8];
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while std::time::Instant::now() < deadline {
        let count = consumer.consume(&mut scratch);
        output.extend_from_slice(&scratch[..count]);
        if matches!(worker.poll_event(), Some(PlaybackEvent::Completed)) {
            break;
        }
        std::thread::yield_now();
    }
    assert!(output.contains(&6.0), "playback must continue past B after clearing");
    assert!(output.contains(&999.0), "playback must reach the end of the file after clearing");
    let _ = worker.join();
}

#[test]
fn ab_repeat_region_longer_than_buffer_seeks_back_to_start() {
    let (worker, mut consumer) = PlaybackWorker::start_with_mode(
        Box::new(RampDecoder::new(1_000)),
        4,
        pulseseek_domain::playback::mode::PlaybackMode::OneShot,
    );
    worker.set_loop_region(Some(loop_region(10, 18))).unwrap();
    finish_loop_region_transition(&mut consumer, 1);

    let mut output = Vec::new();
    let mut scratch = [0.0f32; 8];
    for _ in 0..100_000 {
        let count = consumer.consume(&mut scratch);
        output.extend_from_slice(&scratch[..count]);
        if output.len() >= 16 {
            break;
        }
        std::thread::yield_now();
    }
    output.truncate(16);

    assert_eq!(
        output,
        vec![
            10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0,
            16.0, 17.0
        ]
    );
    assert!(worker.poll_event().is_none());
    control_stop_and_join(worker, consumer);
}

#[test]
fn ab_repeat_long_region_resets_clock_only_when_a_is_audible() {
    let (worker, mut consumer) = PlaybackWorker::start_with_mode(
        Box::new(RampDecoder::new(1_000)),
        4,
        pulseseek_domain::playback::mode::PlaybackMode::OneShot,
    );
    worker.set_loop_region(Some(loop_region(10, 18))).unwrap();

    for expected in 10_u64..18 {
        let mut sample = [0.0f32; 1];
        while consumer.consume_channels(&mut sample, 1, 1) != 1 {
            std::thread::yield_now();
        }
        assert_eq!(sample[0], expected as f32);
        assert_eq!(
            consumer.control().position_frames(),
            expected + 1,
            "clock must reach B before wrapping"
        );
    }

    let mut wrapped = [0.0f32; 1];
    while consumer.consume_channels(&mut wrapped, 1, 1) != 1 {
        std::thread::yield_now();
    }
    assert_eq!(wrapped[0], 10.0);
    assert_eq!(consumer.control().position_frames(), 11);
    control_stop_and_join(worker, consumer);
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
