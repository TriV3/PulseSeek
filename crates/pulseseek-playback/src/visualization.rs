use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use pulseseek_domain::visualization::{
    VisualizationFrame, VisualizationFrameError, MAX_VISUALIZATION_FRAME_SAMPLES,
};
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};

/// Creates one bounded, lock-free stream from the audio callback to one worker.
pub fn visualization_channel(capacity: usize) -> (VisualizationPublisher, VisualizationSubscriber) {
    assert!(capacity > 0, "visualization channel capacity must be positive");
    let (producer, consumer) = HeapRb::new(capacity).split();
    let shutdown = Arc::new(AtomicBool::new(false));
    let dropped = Arc::new(AtomicU64::new(0));
    (
        VisualizationPublisher {
            producer,
            shutdown: Arc::clone(&shutdown),
            dropped: Arc::clone(&dropped),
        },
        VisualizationSubscriber { consumer, shutdown, dropped },
    )
}

pub struct VisualizationPublisher {
    producer: HeapProd<VisualizationFrame>,
    shutdown: Arc<AtomicBool>,
    dropped: Arc<AtomicU64>,
}

impl VisualizationPublisher {
    /// Attempts one publication without waiting, locking, or allocating.
    ///
    /// A full queue drops the incoming frame so playback always wins.
    pub fn try_publish(&mut self, frame: VisualizationFrame) -> PublishResult {
        if self.is_shutdown() {
            return PublishResult::Shutdown;
        }
        if !self.producer.read_is_held() {
            return PublishResult::SubscriberGone;
        }
        match self.producer.try_push(frame) {
            Ok(()) => PublishResult::Published,
            Err(_) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
                PublishResult::DroppedFull
            },
        }
    }

    pub fn dropped_frames(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Release);
    }

    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublishResult {
    Published,
    DroppedFull,
    SubscriberGone,
    Shutdown,
}

pub struct VisualizationSubscriber {
    consumer: HeapCons<VisualizationFrame>,
    shutdown: Arc<AtomicBool>,
    dropped: Arc<AtomicU64>,
}

impl VisualizationSubscriber {
    /// Receives one frame when available without waiting.
    pub fn try_receive(&mut self) -> Option<VisualizationFrame> {
        self.consumer.try_pop()
    }

    pub fn dropped_frames(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }

    pub fn is_closed(&self) -> bool {
        self.is_shutdown() || (!self.consumer.write_is_held() && self.consumer.is_empty())
    }
}

/// Fixed-storage accumulator owned by the playback callback.
pub struct VisualizationTap {
    publisher: VisualizationPublisher,
    sample_rate: u32,
    channels: u16,
    samples_per_frame: usize,
    pending_position_frames: u64,
    pending_len: usize,
    pending: [f32; MAX_VISUALIZATION_FRAME_SAMPLES],
    sequence: u64,
}

impl VisualizationTap {
    pub fn new(
        publisher: VisualizationPublisher,
        sample_rate: u32,
        channels: u16,
        samples_per_frame: usize,
    ) -> Result<Self, VisualizationFrameError> {
        VisualizationFrame::validate_layout(sample_rate, channels, samples_per_frame)?;
        Ok(Self {
            publisher,
            sample_rate,
            channels,
            samples_per_frame,
            pending_position_frames: 0,
            pending_len: 0,
            pending: [0.0; MAX_VISUALIZATION_FRAME_SAMPLES],
            sequence: 0,
        })
    }

    pub(crate) fn capture(
        &mut self,
        samples: &[f32],
        position_frames: u64,
        output_channels: usize,
    ) {
        let channels = usize::from(self.channels);
        if output_channels != channels {
            self.pending_len = 0;
            return;
        }
        let mut consumed = 0;
        while consumed < samples.len() {
            if self.pending_len == 0 {
                self.pending_position_frames = position_frames
                    .saturating_add(u64::try_from(consumed / channels).unwrap_or(u64::MAX));
            }
            let copy_len =
                (self.samples_per_frame - self.pending_len).min(samples.len() - consumed);
            self.pending[self.pending_len..self.pending_len + copy_len]
                .copy_from_slice(&samples[consumed..consumed + copy_len]);
            self.pending_len += copy_len;
            consumed += copy_len;

            if self.pending_len == self.samples_per_frame {
                let frame = VisualizationFrame::new(
                    self.sequence,
                    self.pending_position_frames,
                    self.sample_rate,
                    self.channels,
                    &self.pending[..self.pending_len],
                )
                .expect("visualization tap preserves validated frame shape");
                let _ = self.publisher.try_publish(frame);
                self.sequence = self.sequence.wrapping_add(1);
                self.pending_len = 0;
            }
        }
    }

    pub(crate) fn dropped_frames(&self) -> u64 {
        self.publisher.dropped_frames()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_saturates_frame_positions_at_u64_max() {
        let (publisher, mut subscriber) = visualization_channel(2);
        let mut tap = VisualizationTap::new(publisher, 48_000, 1, 1).unwrap();

        tap.capture(&[0.25, 0.5], u64::MAX, 1);

        assert_eq!(subscriber.try_receive().unwrap().position_frames(), u64::MAX);
        assert_eq!(subscriber.try_receive().unwrap().position_frames(), u64::MAX);
    }

    #[test]
    fn channel_mismatch_discards_a_partial_frame() {
        let (publisher, mut subscriber) = visualization_channel(1);
        let mut tap = VisualizationTap::new(publisher, 48_000, 1, 2).unwrap();

        tap.capture(&[0.1], 0, 1);
        tap.capture(&[0.2, 0.2], 1, 2);
        tap.capture(&[0.3, 0.4], 2, 1);

        let frame = subscriber.try_receive().unwrap();
        assert_eq!(frame.position_frames(), 2);
        assert_eq!(frame.samples(), &[0.3, 0.4]);
    }
}
