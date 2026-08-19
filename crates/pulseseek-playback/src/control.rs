use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use ringbuf::traits::{Consumer, Observer};
use ringbuf::HeapCons;

use crate::analysis_capture::{AnalysisCaptureProducer, MAX_ANALYSIS_CAPTURE_SAMPLES};
use crate::apply_volume;
use crate::engine::BufferedSample;
use crate::visualization::VisualizationTap;

/// Consumer half intended for use by a real-time audio callback.
pub struct PlaybackConsumer {
    pub(crate) consumer: HeapCons<BufferedSample>,
    pub(crate) control: PlaybackControl,
    pub(crate) observed_seek_generation: u64,
    pub(crate) seek_ramp_frame: usize,
    pub(crate) last_output: [f32; MAX_CALLBACK_CHANNELS],
    pub(crate) seek_ramp_origin: [f32; MAX_CALLBACK_CHANNELS],
    pub(crate) seek_fade_out_frame: usize,
    pub(crate) seek_fade_out_origin: [f32; MAX_CALLBACK_CHANNELS],
    pub(crate) buffer_cleared_for_seek: bool,
    pub(crate) visualization_tap: Option<VisualizationTap>,
    pub(crate) monitor_analysis_tap: Option<AnalysisCaptureProducer>,
    pub(crate) analysis_seek_generation: u64,
    pub(crate) analysis_track_change_sequence: u64,
}

pub(crate) const MAX_CALLBACK_CHANNELS: usize = 32;
pub(crate) const SEEK_RAMP_FRAMES: usize = 512;

/// Lock-free playback control shared by the audio owner and callback consumer.
#[derive(Clone)]
pub struct PlaybackControl {
    pub(crate) paused: Arc<AtomicBool>,
    pub(crate) stopped: Arc<AtomicBool>,
    pub(crate) seeking: Arc<AtomicBool>,
    pub(crate) generation: Arc<AtomicU64>,
    pub(crate) seek_generation: Arc<AtomicU64>,
    pub(crate) buffer_discard_request: Arc<AtomicU64>,
    pub(crate) buffer_discard_ack: Arc<AtomicU64>,
    pub(crate) seek_fade_requested: Arc<AtomicBool>,
    pub(crate) seek_fade_complete: Arc<AtomicBool>,
    pub(crate) output_active: Arc<AtomicBool>,
    pub(crate) terminal: Arc<AtomicU8>,
    pub(crate) position_frames: Arc<AtomicU64>,
    pub(crate) next_track_change_sequence: Arc<AtomicU64>,
    pub(crate) reached_track_change_sequence: Arc<AtomicU64>,
    pub(crate) track_changes: Arc<Mutex<VecDeque<PendingTrackChange>>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingTrackChange {
    sequence: u64,
    change: TrackChange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrackChange {
    pub path: String,
    pub duration_ms: Option<u64>,
}

pub(crate) const TERMINAL_ACTIVE: u8 = 0;
pub(crate) const TERMINAL_STOPPED: u8 = 1;
pub(crate) const TERMINAL_COMPLETED: u8 = 2;
pub(crate) const TERMINAL_FAILED: u8 = 3;

impl PlaybackControl {
    /// Pauses consumption without discarding buffered frames.
    pub fn pause(&self) {
        self.paused.store(true, Ordering::Release);
    }

    /// Resumes consumption from the current buffered position.
    pub fn resume(&self) {
        self.paused.store(false, Ordering::Release);
    }

    /// Stops playback permanently for this consumer and worker session.
    pub fn stop(&self) {
        let _ = self.terminal.compare_exchange(
            TERMINAL_ACTIVE,
            TERMINAL_STOPPED,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        self.stopped.store(true, Ordering::Release);
    }

    /// Returns whether consumption is paused.
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Acquire)
    }

    /// Returns whether this playback session has been stopped.
    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }

    /// Returns whether playback reached the end naturally.
    pub fn is_completed(&self) -> bool {
        self.terminal.load(Ordering::Acquire) == TERMINAL_COMPLETED
    }

    /// Returns the number of output frames consumed by the audio callback.
    pub fn position_frames(&self) -> u64 {
        self.position_frames.load(Ordering::Acquire)
    }

    /// Resets the callback clock after a successful seek.
    pub fn set_position_frames(&self, frames: u64) {
        self.position_frames.store(frames, Ordering::Release);
    }

    pub(crate) fn publish_track_change(&self, change: TrackChange) -> u64 {
        let sequence = self.next_track_change_sequence.fetch_add(1, Ordering::AcqRel) + 1;
        if let Ok(mut pending) = self.track_changes.lock() {
            pending.push_back(PendingTrackChange { sequence, change });
        }
        sequence
    }

    pub fn take_track_change(&self) -> Option<TrackChange> {
        let reached = self.reached_track_change_sequence.load(Ordering::Acquire);
        self.track_changes.lock().ok().and_then(|mut pending| {
            if pending.front().is_some_and(|change| change.sequence <= reached) {
                pending.pop_front().map(|change| change.change)
            } else {
                None
            }
        })
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    pub(crate) fn begin_seek(&self) {
        self.seeking.store(true, Ordering::Release);
        if self.paused.load(Ordering::Acquire) || !self.output_active.load(Ordering::Acquire) {
            self.seek_fade_complete.store(true, Ordering::Release);
        } else {
            self.seek_fade_complete.store(false, Ordering::Release);
            self.seek_fade_requested.store(true, Ordering::Release);
        }
    }

    pub(crate) fn wait_for_seek_fade(&self) {
        const MAX_WAIT_STEPS: usize = 200;
        for _ in 0..MAX_WAIT_STEPS {
            if self.seek_fade_complete.load(Ordering::Acquire) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_micros(250));
        }
    }

    pub(crate) fn complete_seek(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.seek_fade_requested.store(false, Ordering::Release);
        self.seeking.store(false, Ordering::Release);
    }

    pub(crate) fn complete_user_seek(&self) {
        self.seek_generation.fetch_add(1, Ordering::AcqRel);
        self.complete_seek();
    }

    pub(crate) fn request_buffer_discard(&self) -> u64 {
        self.buffer_discard_request.fetch_add(1, Ordering::AcqRel) + 1
    }

    pub(crate) fn buffer_discarded(&self, request: u64) -> bool {
        self.buffer_discard_ack.load(Ordering::Acquire) >= request
    }

    pub(crate) fn cancel_seek(&self) {
        self.seek_fade_requested.store(false, Ordering::Release);
        self.seeking.store(false, Ordering::Release);
    }

    pub(crate) fn claim_completion(&self) -> bool {
        if self
            .terminal
            .compare_exchange(
                TERMINAL_ACTIVE,
                TERMINAL_COMPLETED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            self.stopped.store(true, Ordering::Release);
            true
        } else {
            false
        }
    }

    pub(crate) fn claim_failure(&self) -> bool {
        if self
            .terminal
            .compare_exchange(TERMINAL_ACTIVE, TERMINAL_FAILED, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            self.stopped.store(true, Ordering::Release);
            true
        } else {
            false
        }
    }
}

impl PlaybackConsumer {
    /// Reads frames from the ring buffer into `buf`.
    ///
    /// Returns the number of frames actually read (may be less than `buf.len()`).
    /// Intended for the audio callback: no allocation, locking, I/O, or logging.
    pub fn consume(&mut self, buf: &mut [f32]) -> usize {
        if self.acknowledge_buffer_discard() {
            buf.fill(0.0);
            return 0;
        }
        if self.control.is_stopped()
            || self.control.is_paused()
            || self.control.seeking.load(Ordering::Acquire)
        {
            buf.fill(0.0);
            return 0;
        }
        let mut written = 0;
        for sample in buf.iter_mut() {
            let mut buffered = [BufferedSample {
                value: 0.0,
                generation: 0,
                position_reset: None,
                track_change_sequence: None,
            }];
            if self.consume_current(&mut buffered) == 0 {
                break;
            }
            *sample = buffered[0].value;
            written += 1;
        }
        written
    }

    /// Consumes interleaved source frames and maps them to output channels.
    ///
    /// Mono sources are duplicated for multi-channel output. Multi-channel
    /// sources are averaged when output is mono; extra output channels use
    /// silence. No allocation or locking occurs.
    pub(crate) fn consume_channels(
        &mut self,
        buf: &mut [f32],
        source_channels: usize,
        output_channels: usize,
    ) -> usize {
        if source_channels == 0 || output_channels == 0 {
            return 0;
        }
        if self.acknowledge_buffer_discard() {
            buf.fill(0.0);
            return 0;
        }
        if self.control.is_stopped() || self.control.is_paused() {
            buf.fill(0.0);
            return 0;
        }
        if self.control.seeking.load(Ordering::Acquire) {
            self.render_seek_fade(buf, output_channels);
            return 0;
        }

        let track_change_sequence =
            self.control.reached_track_change_sequence.load(Ordering::Acquire);
        if track_change_sequence != self.analysis_track_change_sequence {
            if let Some(tap) = &mut self.monitor_analysis_tap {
                tap.rotate_source();
            }
            self.analysis_track_change_sequence = track_change_sequence;
        }
        let seek_generation = self.control.seek_generation.load(Ordering::Acquire);
        if seek_generation != self.analysis_seek_generation {
            if let Some(tap) = &mut self.monitor_analysis_tap {
                tap.start_new_session_at(self.control.position_frames());
            }
            self.analysis_seek_generation = seek_generation;
        }
        let mut replaced_buffer = false;
        if seek_generation != self.observed_seek_generation {
            replaced_buffer = true;
            self.observed_seek_generation = seek_generation;
            self.seek_ramp_frame = 0;
            // If a large hardware buffer prevented the callback from finishing
            // the requested fade-out before the worker timeout, retain the
            // last sample as the crossfade origin. Starting from zero here
            // would itself create the discontinuity we are trying to remove.
            // After a completed fade `last_output` is already silence.
            self.seek_ramp_origin = self.last_output;
            if !self.buffer_cleared_for_seek {
                self.discard_buffered_samples_fast();
            }
            self.buffer_cleared_for_seek = false;
        }

        let mut written = 0;
        let mut monitor_first_sample = self.control.position_frames();
        let mut monitor_samples = [0.0f32; MAX_ANALYSIS_CAPTURE_SAMPLES];
        let mut monitor_sample_count = 0;
        // Track whether we have exhausted the ring buffer. Once exhausted,
        // fill remaining frames with silence instead of breaking, so the
        // entire cpal output buffer is properly zeroed and avoids a pop or
        // crackle at the end of the stream.
        let mut drained = replaced_buffer;
        for frame in buf.chunks_mut(output_channels) {
            if drained || self.available() < source_channels {
                if self.seek_ramp_frame < SEEK_RAMP_FRAMES {
                    let gain = 1.0 - seek_ramp_progress(self.seek_ramp_frame);
                    for (channel, output) in frame.iter_mut().enumerate() {
                        *output = self.seek_ramp_origin.get(channel).copied().unwrap_or(0.0) * gain;
                    }
                    self.seek_ramp_frame += 1;
                    for (channel, output) in
                        frame.iter().copied().enumerate().take(MAX_CALLBACK_CHANNELS)
                    {
                        self.last_output[channel] = output;
                    }
                } else {
                    frame.fill(0.0);
                }
                drained = true;
                continue;
            }

            let frame_position = self.control.position_frames();
            let mut source_samples = [0.0f32; 32];
            let mut source_sum = 0.0f32;
            let mut complete = true;
            for channel in 0..source_channels {
                let mut sample = [0.0f32];
                if self.consume(&mut sample) != 1 {
                    complete = false;
                    break;
                }
                source_sum += sample[0];
                if channel < source_samples.len() {
                    source_samples[channel] = sample[0];
                }
            }
            if !complete {
                frame.fill(0.0);
                drained = true;
                continue;
            }

            let track_change_sequence =
                self.control.reached_track_change_sequence.load(Ordering::Acquire);
            if track_change_sequence != self.analysis_track_change_sequence {
                if monitor_sample_count > 0 {
                    if let Some(tap) = &mut self.monitor_analysis_tap {
                        let _ = tap.try_capture(
                            monitor_first_sample,
                            &monitor_samples[..monitor_sample_count],
                            false,
                        );
                    }
                    monitor_sample_count = 0;
                    monitor_first_sample = 0;
                }
                if let Some(tap) = &mut self.monitor_analysis_tap {
                    tap.rotate_source();
                }
                self.analysis_track_change_sequence = track_change_sequence;
            }

            let reset_position = self.control.position_frames();
            if reset_position != frame_position && monitor_sample_count > 0 {
                if let Some(tap) = &mut self.monitor_analysis_tap {
                    let _ = tap.try_capture(
                        monitor_first_sample,
                        &monitor_samples[..monitor_sample_count],
                        true,
                    );
                }
                monitor_first_sample = reset_position;
                monitor_sample_count = 0;
            }

            if source_channels == 1 {
                frame.fill(source_samples[0]);
            } else if output_channels == 1 {
                frame[0] = source_sum / source_channels as f32;
            } else {
                for (channel, output) in frame.iter_mut().enumerate() {
                    *output = if channel < source_channels && channel < source_samples.len() {
                        source_samples[channel]
                    } else {
                        0.0
                    };
                }
            }

            if monitor_sample_count + source_channels > monitor_samples.len() {
                if let Some(tap) = &mut self.monitor_analysis_tap {
                    let _ = tap.try_capture(
                        monitor_first_sample,
                        &monitor_samples[..monitor_sample_count],
                        false,
                    );
                }
                monitor_first_sample = self.control.position_frames();
                monitor_sample_count = 0;
            }
            monitor_samples[monitor_sample_count..monitor_sample_count + source_channels]
                .copy_from_slice(&source_samples[..source_channels]);
            monitor_sample_count += source_channels;

            if self.seek_ramp_frame < SEEK_RAMP_FRAMES {
                let progress = seek_ramp_progress(self.seek_ramp_frame);
                let has_signal = frame.iter().any(|sample| sample.abs() > f32::EPSILON);
                for (channel, output) in frame.iter_mut().enumerate() {
                    let origin = self.seek_ramp_origin.get(channel).copied().unwrap_or(0.0);
                    *output = origin + (*output - origin) * progress;
                }
                // Codec seeks may emit silent pre-roll. Do not consume the
                // anti-click ramp until real audio resumes, otherwise the
                // first audible frame can still arrive at full gain.
                if has_signal {
                    self.seek_ramp_frame += 1;
                }
            }
            for (channel, output) in frame.iter().copied().enumerate().take(MAX_CALLBACK_CHANNELS) {
                self.last_output[channel] = output;
            }
            written += frame.len();
            self.control.position_frames.fetch_add(1, Ordering::Relaxed);
            self.control.output_active.store(true, Ordering::Release);
        }
        if monitor_sample_count > 0 {
            if let Some(tap) = &mut self.monitor_analysis_tap {
                let _ = tap.try_capture(
                    monitor_first_sample,
                    &monitor_samples[..monitor_sample_count],
                    false,
                );
            }
        }
        written
    }

    fn render_seek_fade(&mut self, buf: &mut [f32], output_channels: usize) {
        if self.control.seek_fade_requested.swap(false, Ordering::AcqRel) {
            self.seek_fade_out_frame = 0;
            self.seek_fade_out_origin = self.last_output;
        }
        for frame in buf.chunks_mut(output_channels) {
            if self.seek_fade_out_frame < SEEK_RAMP_FRAMES {
                let gain = 1.0 - seek_ramp_progress(self.seek_fade_out_frame);
                for (channel, output) in frame.iter_mut().enumerate() {
                    *output = self.seek_fade_out_origin.get(channel).copied().unwrap_or(0.0) * gain;
                }
                self.seek_fade_out_frame += 1;
            } else {
                frame.fill(0.0);
            }
        }
        if self.seek_fade_out_frame >= SEEK_RAMP_FRAMES {
            self.last_output.fill(0.0);
            if !self.buffer_cleared_for_seek {
                self.discard_buffered_samples_fast();
                self.buffer_cleared_for_seek = true;
            }
            self.control.output_active.store(false, Ordering::Release);
            self.control.seek_fade_complete.store(true, Ordering::Release);
        }
    }

    /// Returns a handle for pausing and resuming this consumer.
    pub fn control(&self) -> PlaybackControl {
        self.control.clone()
    }

    fn consume_current(&mut self, output: &mut [BufferedSample]) -> usize {
        let generation = self.control.generation.load(Ordering::Acquire);
        if self.consumer.pop_slice(output) != 1 || output[0].generation != generation {
            return 0;
        }
        if let Some(frames) = output[0].position_reset {
            self.control.set_position_frames(frames);
        }
        if let Some(sequence) = output[0].track_change_sequence {
            self.control.reached_track_change_sequence.store(sequence, Ordering::Release);
        }
        1
    }

    fn discard_buffered_samples_fast(&mut self) {
        let count = self.consumer.occupied_len();
        self.discard_buffered_samples(count);
    }

    fn acknowledge_buffer_discard(&mut self) -> bool {
        let request = self.control.buffer_discard_request.load(Ordering::Acquire);
        if request <= self.control.buffer_discard_ack.load(Ordering::Acquire) {
            return false;
        }
        self.discard_buffered_samples_fast();
        self.control.buffer_discard_ack.store(request, Ordering::Release);
        true
    }

    pub(crate) fn discard_buffered_samples(&mut self, count: usize) {
        let count = count.min(self.consumer.occupied_len());
        // BufferedSample is Copy and has no destructor. Advancing the SPSC
        // consumer index therefore invalidates the snapshot in constant time
        // without touching every stale sample on the real-time thread.
        unsafe {
            self.consumer.advance_read_index(count);
        }
    }

    /// Maps source channels and applies volume to samples for an output callback.
    ///
    /// Performs only bounded buffer work and arithmetic; no allocation or locking.
    pub fn consume_channels_with_volume(
        &mut self,
        buf: &mut [f32],
        source_channels: usize,
        output_channels: usize,
        gain: f32,
    ) -> usize {
        let written = self.consume_channels(buf, source_channels, output_channels);
        for sample in buf.iter_mut() {
            *sample = apply_volume(*sample, gain);
        }
        if written > 0 {
            let frame_count = written / output_channels;
            let position_frames = self
                .control
                .position_frames()
                .saturating_sub(u64::try_from(frame_count).unwrap_or(u64::MAX));
            if let Some(tap) = &mut self.visualization_tap {
                tap.capture(&buf[..written], position_frames, output_channels);
            }
        }
        written
    }

    pub fn set_monitor_analysis_tap(&mut self, tap: AnalysisCaptureProducer) {
        self.monitor_analysis_tap = Some(tap);
    }

    pub fn clear_monitor_analysis_tap(&mut self) {
        self.monitor_analysis_tap = None;
    }

    /// Installs a preconfigured visualization tap before this consumer enters
    /// the audio callback.
    pub fn set_visualization_tap(&mut self, tap: VisualizationTap) {
        self.visualization_tap = Some(tap);
    }

    pub fn clear_visualization_tap(&mut self) {
        self.visualization_tap = None;
    }

    pub fn visualization_dropped_frames(&self) -> Option<u64> {
        self.visualization_tap.as_ref().map(VisualizationTap::dropped_frames)
    }

    /// Returns the number of frames currently available to the callback.
    pub fn available(&self) -> usize {
        if self.control.is_stopped() {
            return 0;
        }
        self.consumer.occupied_len()
    }
}

fn seek_ramp_progress(frame: usize) -> f32 {
    let linear = (frame + 1) as f32 / SEEK_RAMP_FRAMES as f32;
    linear * linear * (3.0 - 2.0 * linear)
}
