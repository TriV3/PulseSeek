use std::collections::VecDeque;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration as ThreadDuration;

use pulseseek_domain::decoder::{DecodeError, Decoder};
use pulseseek_domain::playback::mode::PlaybackMode;
use pulseseek_domain::playback::position::{Position, SeekTarget};
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};

mod resampling;
use resampling::SampleRateConverter;

/// Applies linear gain to one sample and hard-clips the result to audio range.
///
/// Intended for the real-time output callback: constant-time arithmetic only.
pub fn apply_volume(sample: f32, gain: f32) -> f32 {
    (sample * gain).clamp(-1.0, 1.0)
}

/// Error produced by playback operations.
#[derive(Debug)]
pub struct PlaybackError {
    kind: PlaybackErrorKind,
}

/// Terminal playback outcome for one-shot playback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaybackEvent {
    Completed,
    Failed,
}

#[derive(Debug)]
enum PlaybackErrorKind {
    Decode(DecodeError),
    InvalidFrameCount { returned: usize, capacity: usize },
    WorkerStopped,
    NoFrames,
}

impl fmt::Display for PlaybackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            PlaybackErrorKind::Decode(error) => write!(f, "playback error: {error}"),
            PlaybackErrorKind::InvalidFrameCount { returned, capacity } => {
                write!(f, "decoder returned {returned} frames for buffer capacity {capacity}")
            },
            PlaybackErrorKind::WorkerStopped => write!(f, "playback worker stopped"),
            PlaybackErrorKind::NoFrames => write!(f, "playback source produced no frames"),
        }
    }
}

impl std::error::Error for PlaybackError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            PlaybackErrorKind::Decode(error) => Some(error),
            PlaybackErrorKind::InvalidFrameCount { .. } => None,
            PlaybackErrorKind::WorkerStopped => None,
            PlaybackErrorKind::NoFrames => None,
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
    producer: HeapProd<BufferedSample>,
    buffer_size: usize,
    eof: bool,
    control: PlaybackControl,
    resampler: Option<SampleRateConverter>,
    pending: VecDeque<f32>,
    mode: PlaybackMode,
    cycle_produced: bool,
    /// Total frames written to the ring buffer so far.
    pub frames_written: u64,
}

#[derive(Clone, Copy)]
struct BufferedSample {
    value: f32,
    generation: u64,
}

/// Consumer half intended for use by a real-time audio callback.
pub struct PlaybackConsumer {
    consumer: HeapCons<BufferedSample>,
    control: PlaybackControl,
}

/// Lock-free playback control shared by the audio owner and callback consumer.
#[derive(Clone)]
pub struct PlaybackControl {
    paused: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
    seeking: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
    terminal: Arc<AtomicU8>,
}

const TERMINAL_ACTIVE: u8 = 0;
const TERMINAL_STOPPED: u8 = 1;
const TERMINAL_COMPLETED: u8 = 2;
const TERMINAL_FAILED: u8 = 3;

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

    fn begin_seek(&self) {
        self.seeking.store(true, Ordering::Release);
    }

    fn complete_seek(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.seeking.store(false, Ordering::Release);
    }

    fn cancel_seek(&self) {
        self.seeking.store(false, Ordering::Release);
    }

    fn claim_completion(&self) -> bool {
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

    fn claim_failure(&self) -> bool {
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

/// Decoder worker that keeps the producer half fed until EOF or shutdown.
pub struct PlaybackWorker {
    stop: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
    error: Arc<Mutex<Option<PlaybackError>>>,
    commands: Sender<WorkerCommand>,
    events: Receiver<PlaybackEvent>,
    join: Option<JoinHandle<()>>,
}

enum WorkerCommand {
    Seek { target: SeekTarget, response: SyncSender<Result<Position, PlaybackError>> },
    SetMode { mode: PlaybackMode, response: SyncSender<Result<(), PlaybackError>> },
}

fn execute_worker_command(engine: &mut PlaybackEngine, command: WorkerCommand) -> bool {
    match command {
        WorkerCommand::Seek { target, response } => {
            engine.control.begin_seek();
            let result = engine.decoder.seek(target);
            let succeeded = result.is_ok();
            let result = match result {
                Ok(position) => {
                    engine.eof = false;
                    engine.pending.clear();
                    if let Some(resampler) = &mut engine.resampler {
                        resampler.reset();
                    }
                    engine.control.complete_seek();
                    Ok(position)
                },
                Err(error) => {
                    engine.control.cancel_seek();
                    Err(PlaybackError::from(error))
                },
            };
            let _ = response.send(result);
            succeeded
        },
        WorkerCommand::SetMode { mode, response } => {
            engine.mode = mode;
            let _ = response.send(Ok(()));
            mode == PlaybackMode::LoopCurrent
        },
    }
}

impl PlaybackEngine {
    /// Creates a new playback engine.
    ///
    /// `buffer_frames` is the capacity of the internal ring buffer in frames.
    pub fn new(decoder: Box<dyn Decoder>, buffer_frames: usize) -> (Self, PlaybackConsumer) {
        Self::new_with_mode(decoder, buffer_frames, PlaybackMode::OneShot)
    }

    fn new_with_mode(
        decoder: Box<dyn Decoder>,
        buffer_frames: usize,
        mode: PlaybackMode,
    ) -> (Self, PlaybackConsumer) {
        Self::new_with_resampler_mode(decoder, buffer_frames, None, mode)
    }

    fn new_with_resampler_mode(
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
        let generation = self.control.generation.load(Ordering::Acquire);
        let pushed = self.producer.push_iter(
            buf[..frames].iter().copied().map(|value| BufferedSample { value, generation }),
        );
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

    fn is_buffer_full(&self) -> bool {
        self.producer.vacant_len() == 0
    }

    fn drain_pending(&mut self) {
        let generation = self.control.generation.load(Ordering::Acquire);
        while self.producer.vacant_len() > 0 {
            let Some(value) = self.pending.pop_front() else { break };
            let _ = self.producer.try_push(BufferedSample { value, generation });
            self.frames_written += 1;
            self.cycle_produced = true;
        }
    }

    fn restart_current(&mut self) -> Result<(), PlaybackError> {
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

impl PlaybackWorker {
    /// Starts decoder work on a dedicated non-audio thread.
    pub fn start(decoder: Box<dyn Decoder>, buffer_frames: usize) -> (Self, PlaybackConsumer) {
        Self::start_with_mode(decoder, buffer_frames, PlaybackMode::OneShot)
    }

    /// Starts decoder work with the selected end-of-file mode.
    pub fn start_with_mode(
        decoder: Box<dyn Decoder>,
        buffer_frames: usize,
        mode: PlaybackMode,
    ) -> (Self, PlaybackConsumer) {
        let (engine, consumer) = PlaybackEngine::new_with_mode(decoder, buffer_frames, mode);
        Self::start_engine(engine, consumer)
    }

    /// Starts decoder work with worker-side sample-rate conversion.
    pub fn start_resampled(
        decoder: Box<dyn Decoder>,
        buffer_frames: usize,
        channels: usize,
        source_rate: u32,
        target_rate: u32,
    ) -> Result<(Self, PlaybackConsumer), DecodeError> {
        Self::start_resampled_with_mode(
            decoder,
            buffer_frames,
            channels,
            source_rate,
            target_rate,
            PlaybackMode::OneShot,
        )
    }

    /// Starts decoder work with resampling and the selected end-of-file mode.
    pub fn start_resampled_with_mode(
        decoder: Box<dyn Decoder>,
        buffer_frames: usize,
        channels: usize,
        source_rate: u32,
        target_rate: u32,
        mode: PlaybackMode,
    ) -> Result<(Self, PlaybackConsumer), DecodeError> {
        SampleRateConverter::validate(channels, source_rate, target_rate)?;
        let resampler = if source_rate == target_rate {
            None
        } else {
            Some(SampleRateConverter::new(channels, source_rate, target_rate)?)
        };
        let (engine, consumer) =
            PlaybackEngine::new_with_resampler_mode(decoder, buffer_frames, resampler, mode);
        Ok(Self::start_engine(engine, consumer))
    }

    fn start_engine(
        mut engine: PlaybackEngine,
        consumer: PlaybackConsumer,
    ) -> (Self, PlaybackConsumer) {
        let stop = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        let error = Arc::new(Mutex::new(None));
        let worker_stop = Arc::clone(&stop);
        let worker_finished = Arc::clone(&finished);
        let worker_error = Arc::clone(&error);
        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();

        let join = thread::spawn(move || {
            let mut reached_eof = false;
            while !worker_stop.load(Ordering::Acquire) && !engine.control.is_stopped() {
                while let Ok(command) = command_rx.try_recv() {
                    if execute_worker_command(&mut engine, command) {
                        reached_eof = false;
                        worker_finished.store(false, Ordering::Release);
                    }
                }
                if reached_eof {
                    if engine.mode == PlaybackMode::LoopCurrent {
                        if engine.is_finished() {
                            if !engine.cycle_produced {
                                if engine.control.claim_failure() {
                                    let _ = event_tx.send(PlaybackEvent::Failed);
                                    *worker_error.lock().expect("playback error mutex poisoned") =
                                        Some(PlaybackError { kind: PlaybackErrorKind::NoFrames });
                                }
                                break;
                            }
                            match engine.restart_current() {
                                Ok(()) => {
                                    reached_eof = false;
                                    worker_finished.store(false, Ordering::Release);
                                },
                                Err(playback_error) => {
                                    if engine.control.claim_failure() {
                                        let _ = event_tx.send(PlaybackEvent::Failed);
                                        *worker_error
                                            .lock()
                                            .expect("playback error mutex poisoned") =
                                            Some(playback_error);
                                    }
                                    break;
                                },
                            }
                        } else {
                            thread::yield_now();
                        }
                        continue;
                    }
                    if engine.is_finished() && engine.control.claim_completion() {
                        let _ = event_tx.send(PlaybackEvent::Completed);
                        break;
                    }
                    match command_rx.recv_timeout(ThreadDuration::from_millis(10)) {
                        Ok(command) => {
                            if execute_worker_command(&mut engine, command) {
                                reached_eof = false;
                                worker_finished.store(false, Ordering::Release);
                            }
                        },
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                    continue;
                }
                match engine.process_chunk() {
                    Ok(false) => {
                        reached_eof = true;
                        worker_finished.store(true, Ordering::Release);
                    },
                    Ok(true) if engine.is_buffer_full() => thread::yield_now(),
                    Ok(true) => {},
                    Err(playback_error) => {
                        if engine.control.claim_failure() {
                            let _ = event_tx.send(PlaybackEvent::Failed);
                            *worker_error.lock().expect("playback error mutex poisoned") =
                                Some(playback_error);
                        }
                        break;
                    },
                }
            }
            worker_finished.store(true, Ordering::Release);
        });

        (
            Self {
                stop,
                finished,
                error,
                commands: command_tx,
                events: event_rx,
                join: Some(join),
            },
            consumer,
        )
    }

    /// Returns `true` after worker reached EOF, failed, or was stopped.
    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }

    /// Returns the next terminal event without blocking.
    pub fn poll_event(&self) -> Option<PlaybackEvent> {
        self.events.try_recv().ok()
    }

    /// Waits for worker to reach EOF and returns decoder error, if any.
    pub fn wait(mut self) -> Result<(), PlaybackError> {
        while !self.finished.load(Ordering::Acquire) && !self.stop.load(Ordering::Acquire) {
            thread::yield_now();
        }
        self.stop.store(true, Ordering::Release);
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

    /// Seeks decoder worker to validated target and refills from there.
    pub fn seek(&self, target: SeekTarget) -> Result<Position, PlaybackError> {
        let (response_tx, response_rx) = mpsc::sync_channel(1);
        self.commands
            .send(WorkerCommand::Seek { target, response: response_tx })
            .map_err(|_| PlaybackError { kind: PlaybackErrorKind::WorkerStopped })?;
        response_rx.recv().map_err(|_| PlaybackError { kind: PlaybackErrorKind::WorkerStopped })?
    }

    /// Changes end-of-file behavior on worker thread.
    pub fn set_mode(&self, mode: PlaybackMode) -> Result<(), PlaybackError> {
        let (response_tx, response_rx) = mpsc::sync_channel(1);
        self.commands
            .send(WorkerCommand::SetMode { mode, response: response_tx })
            .map_err(|_| PlaybackError { kind: PlaybackErrorKind::WorkerStopped })?;
        response_rx.recv().map_err(|_| PlaybackError { kind: PlaybackErrorKind::WorkerStopped })?
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
        if self.control.is_stopped()
            || self.control.is_paused()
            || self.control.seeking.load(Ordering::Acquire)
        {
            buf.fill(0.0);
            return 0;
        }
        let mut written = 0;
        for sample in buf.iter_mut() {
            let mut buffered = [BufferedSample { value: 0.0, generation: 0 }];
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
    pub fn consume_channels(
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

    /// Returns a handle for pausing and resuming this consumer.
    pub fn control(&self) -> PlaybackControl {
        self.control.clone()
    }

    fn consume_current(&mut self, output: &mut [BufferedSample]) -> usize {
        let generation = self.control.generation.load(Ordering::Acquire);
        while self.consumer.pop_slice(output) == 1 {
            if output[0].generation == generation {
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
        let (worker, mut consumer) = PlaybackWorker::start_resampled(
            Box::new(RampDecoder::new(441)),
            256,
            1,
            44_100,
            48_000,
        )
        .unwrap();
        let output = collect_worker_output(worker, &mut consumer);

        assert_eq!(output.len(), 480);
        assert!(output.iter().any(|sample| *sample > 0.0));
    }

    #[test]
    fn resampling_48000_to_44100_preserves_duration_ratio() {
        let (worker, mut consumer) = PlaybackWorker::start_resampled(
            Box::new(RampDecoder::new(480)),
            256,
            1,
            48_000,
            44_100,
        )
        .unwrap();
        let output = collect_worker_output(worker, &mut consumer);

        assert_eq!(output.len(), 441);
        assert!(output.iter().any(|sample| *sample > 0.0));
    }

    #[test]
    fn resampling_stereo_keeps_interleaved_channel_sample_count() {
        let (worker, mut consumer) = PlaybackWorker::start_resampled(
            Box::new(RampDecoder::new(882)),
            256,
            2,
            44_100,
            48_000,
        )
        .unwrap();
        let output = collect_worker_output(worker, &mut consumer);

        assert_eq!(output.len(), 960);
    }

    #[test]
    fn resampling_rejects_invalid_configuration() {
        assert!(PlaybackWorker::start_resampled(
            Box::new(RampDecoder::new(4)),
            16,
            0,
            44_100,
            48_000,
        )
        .is_err());
        assert!(PlaybackWorker::start_resampled(Box::new(RampDecoder::new(4)), 16, 1, 0, 48_000,)
            .is_err());
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
        let (worker, mut consumer) = PlaybackWorker::start_resampled(
            Box::new(RampDecoder::new(441)),
            256,
            1,
            44_100,
            48_000,
        )
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
            std::thread::sleep(ThreadDuration::from_millis(1));
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
}
