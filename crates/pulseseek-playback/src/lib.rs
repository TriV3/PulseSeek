use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use pulseseek_domain::decoder::{DecodeError, Decoder};
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};

/// Error produced by playback operations.
#[derive(Debug)]
pub struct PlaybackError {
    kind: PlaybackErrorKind,
}

#[derive(Debug)]
enum PlaybackErrorKind {
    Decode(DecodeError),
    InvalidFrameCount { returned: usize, capacity: usize },
}

impl fmt::Display for PlaybackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            PlaybackErrorKind::Decode(error) => write!(f, "playback error: {error}"),
            PlaybackErrorKind::InvalidFrameCount { returned, capacity } => {
                write!(f, "decoder returned {returned} frames for buffer capacity {capacity}")
            },
        }
    }
}

impl std::error::Error for PlaybackError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            PlaybackErrorKind::Decode(error) => Some(error),
            PlaybackErrorKind::InvalidFrameCount { .. } => None,
        }
    }
}

impl From<DecodeError> for PlaybackError {
    fn from(e: DecodeError) -> Self {
        Self { kind: PlaybackErrorKind::Decode(e) }
    }
}

/// Engine that streams decoded audio frames through a ring buffer.
///
/// Reads from a `Decoder` and makes frames available via the ring buffer
/// for a real-time audio callback to consume.
pub struct PlaybackEngine {
    decoder: Box<dyn Decoder>,
    producer: HeapProd<f32>,
    buffer_size: usize,
    eof: bool,
    /// Total frames written to the ring buffer so far.
    pub frames_written: u64,
}

/// Consumer half intended for use by a real-time audio callback.
pub struct PlaybackConsumer {
    consumer: HeapCons<f32>,
}

/// Decoder worker that keeps the producer half fed until EOF or shutdown.
pub struct PlaybackWorker {
    stop: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
    error: Arc<Mutex<Option<PlaybackError>>>,
    join: Option<JoinHandle<()>>,
}

impl PlaybackEngine {
    /// Creates a new playback engine.
    ///
    /// `buffer_frames` is the capacity of the internal ring buffer in frames.
    pub fn new(decoder: Box<dyn Decoder>, buffer_frames: usize) -> (Self, PlaybackConsumer) {
        assert!(buffer_frames > 0, "playback buffer must contain at least one frame");
        let (producer, consumer) = HeapRb::new(buffer_frames).split();
        (
            Self { decoder, producer, buffer_size: buffer_frames, eof: false, frames_written: 0 },
            PlaybackConsumer { consumer },
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

        let available = self.producer.vacant_len();
        if available == 0 {
            return Ok(true);
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
        let pushed = self.producer.push_slice(&buf[..frames]);
        self.frames_written += pushed as u64;

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

    fn is_buffer_full(&self) -> bool {
        self.producer.vacant_len() == 0
    }
}

impl PlaybackWorker {
    /// Starts decoder work on a dedicated non-audio thread.
    pub fn start(decoder: Box<dyn Decoder>, buffer_frames: usize) -> (Self, PlaybackConsumer) {
        let (mut engine, consumer) = PlaybackEngine::new(decoder, buffer_frames);
        let stop = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        let error = Arc::new(Mutex::new(None));
        let worker_stop = Arc::clone(&stop);
        let worker_finished = Arc::clone(&finished);
        let worker_error = Arc::clone(&error);

        let join = thread::spawn(move || {
            while !worker_stop.load(Ordering::Acquire) {
                match engine.process_chunk() {
                    Ok(false) => break,
                    Ok(true) if engine.is_buffer_full() => thread::yield_now(),
                    Ok(true) => {},
                    Err(playback_error) => {
                        *worker_error.lock().expect("playback error mutex poisoned") =
                            Some(playback_error);
                        break;
                    },
                }
            }
            worker_finished.store(true, Ordering::Release);
        });

        (Self { stop, finished, error, join: Some(join) }, consumer)
    }

    /// Returns `true` after worker reached EOF, failed, or was stopped.
    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }

    /// Waits for worker to reach EOF and returns decoder error, if any.
    pub fn wait(mut self) -> Result<(), PlaybackError> {
        self.join
            .take()
            .expect("playback worker already joined")
            .join()
            .expect("playback worker panicked");
        match self.error.lock().expect("playback error mutex poisoned").take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    /// Stops worker immediately and returns decoder error, if any.
    pub fn join(self) -> Result<(), PlaybackError> {
        self.stop.store(true, Ordering::Release);
        self.wait()
    }
}

impl Drop for PlaybackWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl PlaybackConsumer {
    /// Reads frames from the ring buffer into `buf`.
    ///
    /// Returns the number of frames actually read (may be less than `buf.len()`).
    /// Intended for the audio callback: no allocation, locking, I/O, or logging.
    pub fn consume(&mut self, buf: &mut [f32]) -> usize {
        let available = self.consumer.occupied_len().min(buf.len());
        if available == 0 {
            return 0;
        }
        self.consumer.pop_slice(&mut buf[..available])
    }

    /// Consumes interleaved source frames and maps them to output channels.
    ///
    /// Mono sources are duplicated for multi-channel output. Multi-channel
    /// sources are averaged when output is mono; extra output channels use
    /// silence. No allocation or locking occurs.
    pub fn consume_channels(
        &mut self,
        buf: &mut [f32],
        source_channels: usize,
        output_channels: usize,
    ) -> usize {
        if source_channels == 0 || output_channels == 0 {
            return 0;
        }

        let mut written = 0;
        for frame in buf.chunks_mut(output_channels) {
            if self.available() < source_channels {
                frame.fill(0.0);
                break;
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
                break;
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
        }
        written
    }

    /// Returns the number of frames currently available to the callback.
    pub fn available(&self) -> usize {
        self.consumer.occupied_len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            _target: pulseseek_domain::playback::position::SeekTarget,
        ) -> Result<pulseseek_domain::playback::position::Position, DecodeError> {
            unimplemented!("not used in tests")
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

        for _ in 0..10_000 {
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
}
