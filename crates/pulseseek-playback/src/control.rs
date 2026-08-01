use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;

use ringbuf::traits::{Consumer, Observer};
use ringbuf::HeapCons;

use crate::apply_volume;
use crate::engine::BufferedSample;

/// Consumer half intended for use by a real-time audio callback.
pub struct PlaybackConsumer {
    pub(crate) consumer: HeapCons<BufferedSample>,
    pub(crate) control: PlaybackControl,
}

/// Lock-free playback control shared by the audio owner and callback consumer.
#[derive(Clone)]
pub struct PlaybackControl {
    pub(crate) paused: Arc<AtomicBool>,
    pub(crate) stopped: Arc<AtomicBool>,
    pub(crate) seeking: Arc<AtomicBool>,
    pub(crate) generation: Arc<AtomicU64>,
    pub(crate) terminal: Arc<AtomicU8>,
    pub(crate) position_frames: Arc<AtomicU64>,
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

    pub(crate) fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    pub(crate) fn begin_seek(&self) {
        self.seeking.store(true, Ordering::Release);
    }

    pub(crate) fn complete_seek(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.seeking.store(false, Ordering::Release);
    }

    pub(crate) fn cancel_seek(&self) {
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
        if self.control.is_stopped()
            || self.control.is_paused()
            || self.control.seeking.load(Ordering::Acquire)
        {
            buf.fill(0.0);
            return 0;
        }
        let mut written = 0;
        for sample in buf.iter_mut() {
            let mut buffered =
                [BufferedSample { value: 0.0, generation: 0, resets_position: false }];
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
        if self.control.is_stopped()
            || self.control.is_paused()
            || self.control.seeking.load(Ordering::Acquire)
        {
            buf.fill(0.0);
            return 0;
        }

        let mut written = 0;
        // Track whether we have exhausted the ring buffer. Once exhausted,
        // fill remaining frames with silence instead of breaking, so the
        // entire cpal output buffer is properly zeroed and avoids a pop or
        // crackle at the end of the stream.
        let mut drained = false;
        for frame in buf.chunks_mut(output_channels) {
            if drained || self.available() < source_channels {
                frame.fill(0.0);
                drained = true;
                continue;
            }

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
            written += frame.len();
            self.control.position_frames.fetch_add(1, Ordering::Relaxed);
        }
        written
    }

    /// Returns a handle for pausing and resuming this consumer.
    pub fn control(&self) -> PlaybackControl {
        self.control.clone()
    }

    fn consume_current(&mut self, output: &mut [BufferedSample]) -> usize {
        let generation = self.control.generation.load(Ordering::Acquire);
        while self.consumer.pop_slice(output) == 1 {
            if output[0].generation == generation {
                if output[0].resets_position {
                    self.control.set_position_frames(0);
                }
                return 1;
            }
        }
        0
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
        written
    }

    /// Returns the number of frames currently available to the callback.
    pub fn available(&self) -> usize {
        if self.control.is_stopped() {
            return 0;
        }
        self.consumer.occupied_len()
    }
}
