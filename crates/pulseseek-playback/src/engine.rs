use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8};
use std::sync::Arc;

use pulseseek_domain::decoder::Decoder;
use pulseseek_domain::playback::mode::PlaybackMode;
use pulseseek_domain::playback::position::Position;
use ringbuf::traits::{Observer, Producer, Split};
use ringbuf::{HeapProd, HeapRb};

use crate::control::{PlaybackConsumer, PlaybackControl, TERMINAL_ACTIVE};
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
    pub(crate) cycle_produced: bool,
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
}

impl PlaybackEngine {
    /// Creates a new playback engine.
    ///
    /// `buffer_frames` is the capacity of the internal ring buffer in frames.
    pub fn new(decoder: Box<dyn Decoder>, buffer_frames: usize) -> (Self, PlaybackConsumer) {
        Self::new_with_mode(decoder, buffer_frames, PlaybackMode::OneShot)
    }

    pub(crate) fn new_with_mode(
        decoder: Box<dyn Decoder>,
        buffer_frames: usize,
        mode: PlaybackMode,
    ) -> (Self, PlaybackConsumer) {
        Self::new_with_resampler_mode(decoder, buffer_frames, None, mode)
    }

    pub(crate) fn new_with_resampler_mode(
        decoder: Box<dyn Decoder>,
        buffer_frames: usize,
        resampler: Option<SampleRateConverter>,
        mode: PlaybackMode,
    ) -> (Self, PlaybackConsumer) {
        assert!(buffer_frames > 0, "playback buffer must contain at least one frame");
        let (producer, consumer) = HeapRb::new(buffer_frames).split();
        let control = PlaybackControl {
            paused: Arc::new(AtomicBool::new(false)),
            stopped: Arc::new(AtomicBool::new(false)),
            seeking: Arc::new(AtomicBool::new(false)),
            generation: Arc::new(AtomicU64::new(0)),
            terminal: Arc::new(AtomicU8::new(TERMINAL_ACTIVE)),
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
                cycle_produced: false,
                loop_cache: Vec::new(),
                loop_cache_overflowed: false,
                loop_cache_offset: 0,
                frames_written: 0,
            },
            PlaybackConsumer { consumer, control },
        )
    }

    /// Reads one chunk from the decoder and pushes frames into the ring buffer.
    ///
    /// Returns `Ok(true)` if more data may be available,
    /// `Ok(false)` if the decoder is exhausted (EOF).
    /// On EOF the ring buffer may still hold unread frames.
    pub fn process_chunk(&mut self) -> Result<bool, PlaybackError> {
        if self.eof {
            return Ok(false);
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
                Some(samples) => self.pending.extend(samples),
                None => self.eof = true,
            }
            self.drain_pending();
            return Ok(!self.eof || !self.pending.is_empty());
        }

        let mut buf = vec![0.0f32; self.buffer_size.min(available)];
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

        // Push available frames into the ring buffer (non-blocking).
        let generation = self.control.generation();
        let pushed = self.producer.push_iter(
            buf[..frames].iter().copied().map(|value| BufferedSample { value, generation }),
        );
        self.cache_cycle_samples(&buf[..pushed]);
        self.frames_written += pushed as u64;
        self.cycle_produced |= pushed > 0;

        Ok(true)
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

    fn drain_pending(&mut self) {
        let generation = self.control.generation();
        while self.producer.vacant_len() > 0 {
            let Some(value) = self.pending.pop_front() else { break };
            let _ = self.producer.try_push(BufferedSample { value, generation });
            self.cache_cycle_sample(value);
            self.frames_written += 1;
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
            let value = self.loop_cache[self.loop_cache_offset];
            self.loop_cache_offset = (self.loop_cache_offset + 1) % self.loop_cache.len();
            let _ = self.producer.try_push(BufferedSample { value, generation });
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
                self.control.complete_seek();
                Ok(())
            },
            Err(error) => {
                self.control.cancel_seek();
                Err(PlaybackError::from(error))
            },
        }
    }
}
