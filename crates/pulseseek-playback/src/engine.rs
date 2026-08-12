use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8};
use std::sync::Arc;

use pulseseek_domain::decoder::Decoder;
use pulseseek_domain::playback::loop_region::LoopRegion;
use pulseseek_domain::playback::mode::PlaybackMode;
use pulseseek_domain::playback::position::Position;
use ringbuf::traits::{Observer, Producer, Split};
use ringbuf::{HeapProd, HeapRb};

use crate::control::{
    PlaybackConsumer, PlaybackControl, MAX_CALLBACK_CHANNELS, SEEK_RAMP_FRAMES, TERMINAL_ACTIVE,
};
use crate::error::*;
use crate::resampling::SampleRateConverter;

/// for a real-time audio callback to consume.
pub struct PlaybackEngine {
    pub(crate) decoder: Box<dyn Decoder>,
    pub(crate) producer: HeapProd<BufferedSample>,
    pub(crate) buffer_size: usize,
    pub(crate) eof: bool,
    pub(crate) control: PlaybackControl,
    pub(crate) resampler: Option<SampleRateConverter>,
    pub(crate) pending: VecDeque<f32>,
    pub(crate) mode: PlaybackMode,
    pub(crate) sample_rate: u32,
    pub(crate) channels: usize,
    pub(crate) loop_region: Option<LoopRegion>,
    pub(crate) region_produced_samples: u64,
    pub(crate) region_boundary_reached: bool,
    /// Position reset marker to attach to the next produced sample, used to
    /// return the consumer clock to the region start after a wrap.
    pub(crate) pending_position_reset: Option<u64>,
    /// Decoder position in milliseconds, tracked so the region fast path can
    /// skip a redundant seek deterministically.
    pub(crate) decoder_position_ms: u64,
    pub(crate) cycle_produced: bool,
    pub(crate) pending_buffer_discard: Option<u64>,
    pub(crate) loop_cache: Vec<f32>,
    pub(crate) loop_cache_overflowed: bool,
    pub(crate) loop_cache_offset: usize,
    /// Total frames written to the ring buffer so far.
    pub frames_written: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct BufferedSample {
    pub(crate) value: f32,
    pub(crate) generation: u64,
    /// When `Some(frames)`, the consumer resets its position clock to
    /// `frames` when this sample is consumed (A–B wrap marker).
    pub(crate) position_reset: Option<u64>,
}

impl PlaybackEngine {
    /// Creates a new playback engine.
    ///
    /// `buffer_frames` is the capacity of the internal ring buffer in frames.
    ///
    /// The engine defaults to a 1000 Hz frame clock so that one produced
    /// frame equals one millisecond, which matches the hand-written fakes
    /// used by the crate tests. Production sessions start through
    /// [`PlaybackWorker::start_resampled_with_mode`], which supplies the real
    /// output sample rate for millisecond-to-frame conversion.
    pub fn new(decoder: Box<dyn Decoder>, buffer_frames: usize) -> (Self, PlaybackConsumer) {
        Self::new_with_mode(decoder, buffer_frames, PlaybackMode::OneShot)
    }

    pub(crate) fn new_with_mode(
        decoder: Box<dyn Decoder>,
        buffer_frames: usize,
        mode: PlaybackMode,
    ) -> (Self, PlaybackConsumer) {
        Self::new_with_resampler_mode(decoder, buffer_frames, None, mode, 1_000, 1)
    }

    pub(crate) fn new_with_resampler_mode(
        decoder: Box<dyn Decoder>,
        buffer_frames: usize,
        resampler: Option<SampleRateConverter>,
        mode: PlaybackMode,
        sample_rate: u32,
        channels: usize,
    ) -> (Self, PlaybackConsumer) {
        assert!(buffer_frames > 0, "playback buffer must contain at least one frame");
        let (producer, consumer) = HeapRb::new(buffer_frames).split();
        let control = PlaybackControl {
            paused: Arc::new(AtomicBool::new(false)),
            stopped: Arc::new(AtomicBool::new(false)),
            seeking: Arc::new(AtomicBool::new(false)),
            generation: Arc::new(AtomicU64::new(0)),
            seek_generation: Arc::new(AtomicU64::new(0)),
            buffer_discard_request: Arc::new(AtomicU64::new(0)),
            buffer_discard_ack: Arc::new(AtomicU64::new(0)),
            seek_fade_requested: Arc::new(AtomicBool::new(false)),
            seek_fade_complete: Arc::new(AtomicBool::new(true)),
            output_active: Arc::new(AtomicBool::new(false)),
            terminal: Arc::new(AtomicU8::new(TERMINAL_ACTIVE)),
            position_frames: Arc::new(AtomicU64::new(0)),
        };
        (
            Self {
                decoder,
                producer,
                buffer_size: buffer_frames,
                eof: false,
                control: control.clone(),
                resampler,
                pending: VecDeque::new(),
                mode,
                sample_rate,
                channels,
                loop_region: None,
                region_produced_samples: 0,
                region_boundary_reached: false,
                pending_position_reset: None,
                decoder_position_ms: 0,
                cycle_produced: false,
                pending_buffer_discard: None,
                loop_cache: Vec::new(),
                loop_cache_overflowed: false,
                loop_cache_offset: 0,
                frames_written: 0,
            },
            PlaybackConsumer {
                consumer,
                control,
                observed_seek_generation: 0,
                seek_ramp_frame: SEEK_RAMP_FRAMES,
                last_output: [0.0; MAX_CALLBACK_CHANNELS],
                seek_ramp_origin: [0.0; MAX_CALLBACK_CHANNELS],
                seek_fade_out_frame: SEEK_RAMP_FRAMES,
                seek_fade_out_origin: [0.0; MAX_CALLBACK_CHANNELS],
                buffer_cleared_for_seek: false,
                visualization_tap: None,
            },
        )
    }

    /// Returns the length of the active loop region in interleaved samples.
    fn region_length_samples(&self) -> u64 {
        let Some(region) = self.loop_region else { return 0 };
        if self.sample_rate == 0 {
            return 0;
        }
        let length_ms = region.end().as_millis().saturating_sub(region.start().as_millis());
        let frames = length_ms.saturating_mul(u64::from(self.sample_rate)) / 1_000;
        frames.saturating_mul(self.channels as u64)
    }

    /// Returns the region start in produced frames.
    fn region_start_frames(&self) -> u64 {
        let Some(region) = self.loop_region else { return 0 };
        if self.sample_rate == 0 {
            return 0;
        }
        region.start().as_millis().saturating_mul(u64::from(self.sample_rate)) / 1_000
    }

    /// Pushes produced samples into the ring buffer, honoring the active
    /// A–B region: production stops at the region end so the worker can wrap
    /// back to the region start.
    fn push_buffered(&mut self, samples: &[f32]) -> usize {
        let generation = self.control.generation();
        let mut pushed = 0;
        for &value in samples {
            if self.region_boundary_reached {
                break;
            }
            let region_len = self.region_length_samples();
            if self.loop_region.is_some() && self.region_produced_samples >= region_len {
                self.region_boundary_reached = true;
                break;
            }
            let position_reset = self.pending_position_reset.take();
            if self.producer.try_push(BufferedSample { value, generation, position_reset }).is_err()
            {
                if position_reset.is_some() {
                    self.pending_position_reset = position_reset;
                }
                break;
            }
            pushed += 1;
            self.frames_written += 1;
            if self.loop_region.is_some() {
                self.region_produced_samples += 1;
                if self.region_produced_samples >= region_len {
                    self.region_boundary_reached = true;
                }
            }
            self.cache_cycle_sample(value);
        }
        pushed
    }

    /// Reads one chunk from the decoder and pushes frames into the ring buffer.
    ///
    /// Returns `Ok(true)` if more data may be available,
    /// `Ok(false)` if the decoder is exhausted (EOF) or the active A–B
    /// region boundary was reached.
    /// On EOF the ring buffer may still hold unread frames.
    pub fn process_chunk(&mut self) -> Result<bool, PlaybackError> {
        if self.eof || self.region_boundary_reached {
            return Ok(false);
        }

        if let Some(request) = self.pending_buffer_discard {
            if !self.control.buffer_discarded(request) {
                return Ok(true);
            }
            self.pending_buffer_discard = None;
        }

        self.drain_pending();
        if !self.pending.is_empty() {
            return Ok(true);
        }

        let available = self.producer.vacant_len();
        if available == 0 {
            return Ok(true);
        }

        if let Some(resampler) = &mut self.resampler {
            match resampler.next_chunk(&mut *self.decoder)? {
                Some(samples) => {
                    self.decoder_position_ms = resampler.source_position_ms();
                    self.pending.extend(samples);
                },
                None => self.eof = true,
            }
            self.drain_pending();
            return Ok(!self.eof && !self.region_boundary_reached || !self.pending.is_empty());
        }

        // Cap decoder reads at the region boundary so a wrap or a clear never
        // discards an unbounded read-ahead beyond B.
        let capacity = self.buffer_size.min(available);
        let read_cap = if self.loop_region.is_some() {
            let remaining =
                self.region_length_samples().saturating_sub(self.region_produced_samples);
            capacity.min(remaining as usize)
        } else {
            capacity
        };
        let mut buf = vec![0.0f32; read_cap];
        let frames = self.decoder.read(&mut buf)?;

        if frames > buf.len() {
            return Err(PlaybackError {
                kind: PlaybackErrorKind::InvalidFrameCount {
                    returned: frames,
                    capacity: buf.len(),
                },
            });
        }

        if frames == 0 {
            self.eof = true;
            return Ok(false);
        }

        if self.sample_rate != 0 {
            self.decoder_position_ms +=
                (frames as u64).saturating_mul(1_000) / u64::from(self.sample_rate);
        }

        // Push available frames into the ring buffer (non-blocking),
        // stopping at the region boundary when one is active.
        let pushed = self.push_buffered(&buf[..frames]);
        self.cycle_produced |= pushed > 0;

        Ok(!self.region_boundary_reached)
    }

    /// Returns the number of frames the decoder has made available.
    pub fn available(&self) -> usize {
        self.producer.occupied_len()
    }

    /// Returns `true` when decoding reached EOF and all produced frames were consumed.
    pub fn is_finished(&self) -> bool {
        self.eof && self.producer.occupied_len() == 0
    }

    pub(crate) fn is_buffer_full(&self) -> bool {
        self.producer.vacant_len() == 0
    }

    pub(crate) fn is_waiting_for_buffer_discard(&self) -> bool {
        self.pending_buffer_discard.is_some_and(|request| !self.control.buffer_discarded(request))
    }

    fn drain_pending(&mut self) {
        while self.producer.vacant_len() > 0 && !self.region_boundary_reached {
            let Some(value) = self.pending.pop_front() else { break };
            let pushed = self.push_buffered(std::slice::from_ref(&value));
            if pushed == 0 {
                break;
            }
            self.cycle_produced = true;
        }
    }

    fn cache_cycle_samples(&mut self, samples: &[f32]) {
        if self.loop_cache_overflowed {
            return;
        }
        if self.loop_cache.len() + samples.len() > self.buffer_size {
            self.loop_cache.clear();
            self.loop_cache_overflowed = true;
            return;
        }
        self.loop_cache.extend_from_slice(samples);
    }

    fn cache_cycle_sample(&mut self, sample: f32) {
        self.cache_cycle_samples(std::slice::from_ref(&sample));
    }

    pub(crate) fn has_cached_cycle(&self) -> bool {
        !self.loop_cache.is_empty() && !self.loop_cache_overflowed
    }

    pub(crate) fn replay_cached_cycle(&mut self) -> bool {
        if !self.has_cached_cycle() {
            return false;
        }
        let generation = self.control.generation();
        let mut pushed = false;
        while self.producer.vacant_len() > 0 {
            let position_reset =
                if self.loop_cache_offset == 0 { Some(self.region_start_frames()) } else { None };
            let value = self.loop_cache[self.loop_cache_offset];
            self.loop_cache_offset = (self.loop_cache_offset + 1) % self.loop_cache.len();
            let _ = self.producer.try_push(BufferedSample { value, generation, position_reset });
            self.frames_written += 1;
            pushed = true;
        }
        pushed
    }

    pub(crate) fn restart_current(&mut self) -> Result<(), PlaybackError> {
        let target = pulseseek_domain::playback::position::Duration::Unknown
            .seek_to(Position::from_millis(0))
            .expect("zero is valid seek target");
        self.control.begin_seek();
        match self.decoder.seek(target) {
            Ok(_) => {
                self.eof = false;
                self.pending.clear();
                self.cycle_produced = false;
                if let Some(resampler) = &mut self.resampler {
                    resampler.reset();
                }
                self.control.set_position_frames(0);
                self.control.complete_seek();
                Ok(())
            },
            Err(error) => {
                self.control.cancel_seek();
                Err(PlaybackError::from(error))
            },
        }
    }

    /// Activates an A–B region and positions the decoder at its start.
    ///
    /// The initial positioning seek is skipped when the decoder is already
    /// at the region start (a fresh worker with `A == 0`), which lets short
    /// regions replay purely from the prebuffer without any decoder seek.
    pub(crate) fn set_loop_region(&mut self, region: LoopRegion) -> Result<(), PlaybackError> {
        if region.start().as_millis() == 0 && self.decoder_position_ms == 0 {
            self.commit_loop_region(region);
            if let Some(resampler) = &mut self.resampler {
                resampler.reset();
            }
            return Ok(());
        }
        let target = pulseseek_domain::playback::position::Duration::Unknown
            .seek_to(region.start())
            .expect("region start is a valid seek target");
        self.control.begin_seek();
        self.control.wait_for_seek_fade();
        match self.decoder.seek(target) {
            Ok(position) => {
                self.commit_loop_region(region);
                self.decoder_position_ms = position.as_millis();
                self.control.set_position_frames(self.region_start_frames());
                self.control.complete_seek();
                if let Some(resampler) = &mut self.resampler {
                    resampler.reset();
                }
                Ok(())
            },
            Err(error) => {
                self.control.cancel_seek();
                Err(PlaybackError::from(error))
            },
        }
    }

    fn commit_loop_region(&mut self, region: LoopRegion) {
        self.pending_buffer_discard = Some(self.control.request_buffer_discard());
        self.loop_region = Some(region);
        self.region_produced_samples = 0;
        self.region_boundary_reached = false;
        self.cycle_produced = false;
        self.loop_cache.clear();
        self.loop_cache_overflowed = false;
        self.loop_cache_offset = 0;
        self.eof = false;
        self.pending.clear();
    }

    /// Deactivates the A–B region without repositioning the decoder.
    ///
    /// Playback continues from the current position under the selected
    /// end-of-file mode.
    pub(crate) fn clear_loop_region(&mut self) {
        self.loop_region = None;
        self.region_produced_samples = 0;
        self.region_boundary_reached = false;
    }

    /// Rebases region progress after a user seek to a position inside the
    /// region. The prebuffer is invalidated so the next wrap returns to the
    /// true region start instead of the seek point.
    pub(crate) fn rebase_loop_region(&mut self, seek_position_ms: u64) {
        let Some(region) = self.loop_region else { return };
        let start_ms = region.start().as_millis();
        let offset_ms = seek_position_ms.saturating_sub(start_ms);
        let frames = if self.sample_rate == 0 {
            0
        } else {
            offset_ms.saturating_mul(u64::from(self.sample_rate)) / 1_000
        };
        self.region_produced_samples = frames.saturating_mul(self.channels as u64);
        self.region_boundary_reached = false;
        self.loop_cache.clear();
        self.loop_cache_overflowed = true;
        self.loop_cache_offset = 0;
    }

    /// Returns whether a seek target lies inside the active region
    /// (half-open `[A, B)`).
    pub(crate) fn loop_region_contains_ms(&self, position_ms: u64) -> bool {
        self.loop_region.is_some_and(|region| {
            let position = Position::from_millis(position_ms);
            region.contains(position)
        })
    }

    /// Seeks the decoder back to the region start after a wrap when the
    /// region is too long to replay from the prebuffer.
    ///
    /// Unlike a user seek, the wrap does not bump the generation: the ring
    /// buffer may still hold the tail of the previous cycle, and the callback
    /// consumes it contiguously before the refill resumes at the region
    /// start.
    pub(crate) fn restart_region(&mut self) -> Result<(), PlaybackError> {
        let Some(region) = self.loop_region else {
            return Err(PlaybackError { kind: PlaybackErrorKind::NoFrames });
        };
        let target = pulseseek_domain::playback::position::Duration::Unknown
            .seek_to(region.start())
            .expect("region start is a valid seek target");
        match self.decoder.seek(target) {
            Ok(position) => {
                self.eof = false;
                self.pending.clear();
                self.cycle_produced = false;
                self.region_produced_samples = 0;
                self.region_boundary_reached = false;
                self.decoder_position_ms = position.as_millis();
                if let Some(resampler) = &mut self.resampler {
                    resampler.reset();
                }
                self.pending_position_reset = Some(self.region_start_frames());
                Ok(())
            },
            Err(error) => Err(PlaybackError::from(error)),
        }
    }
}
