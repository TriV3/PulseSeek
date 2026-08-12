use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use pulseseek_domain::decoder::{DecodeError, Decoder};
use pulseseek_domain::playback::loop_region::LoopRegion;
use pulseseek_domain::playback::mode::PlaybackMode;
use pulseseek_domain::playback::position::{Position, SeekTarget};

use crate::control::PlaybackConsumer;
use crate::engine::PlaybackEngine;
use crate::error::*;
use crate::event::PlaybackEvent;
use crate::resampling::SampleRateConverter;

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
    SetLoopRegion { region: Option<LoopRegion>, response: SyncSender<Result<(), PlaybackError>> },
}

fn execute_worker_command(engine: &mut PlaybackEngine, command: WorkerCommand) -> bool {
    match command {
        WorkerCommand::Seek { target, response } => {
            engine.control.begin_seek();
            engine.control.wait_for_seek_fade();
            let result = engine.decoder.seek(target);
            let succeeded = result.is_ok();
            let result = match result {
                Ok(position) => {
                    engine.eof = false;
                    engine.pending.clear();
                    engine.loop_cache.clear();
                    engine.loop_cache_overflowed = false;
                    engine.loop_cache_offset = 0;
                    engine.decoder_position_ms = position.as_millis();
                    if let Some(resampler) = &mut engine.resampler {
                        resampler.reset();
                    }
                    // A seek inside the active A–B region keeps the loop and
                    // rebases its progress; a seek outside it disables the
                    // region so the user is never trapped inside the loop.
                    if engine.loop_region.is_some() {
                        let position_ms = position.as_millis();
                        if engine.loop_region_contains_ms(position_ms) {
                            engine.rebase_loop_region(position_ms);
                        } else {
                            engine.clear_loop_region();
                        }
                    }
                    engine.control.complete_user_seek();
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
        WorkerCommand::SetLoopRegion { region, response } => {
            let result = match region {
                Some(region) => engine.set_loop_region(region),
                None => {
                    engine.clear_loop_region();
                    Ok(())
                },
            };
            let _ = response.send(result);
            true
        },
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
        let (engine, consumer) = PlaybackEngine::new_with_resampler_mode(
            decoder,
            buffer_frames,
            resampler,
            mode,
            target_rate,
            channels,
        );
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
                    if engine.loop_region.is_some() && !engine.eof {
                        // A–B region boundary: wrap back to the region start.
                        // Short regions replay from the prebuffer; long ones
                        // seek the decoder back to A.
                        if engine.has_cached_cycle() {
                            worker_finished.store(false, Ordering::Release);
                            if !engine.replay_cached_cycle() {
                                thread::yield_now();
                            }
                            continue;
                        }
                        if !engine.cycle_produced {
                            if engine.control.claim_failure() {
                                let _ = event_tx.send(PlaybackEvent::Failed);
                                *worker_error.lock().expect("playback error mutex poisoned") =
                                    Some(PlaybackError { kind: PlaybackErrorKind::NoFrames });
                            }
                            break;
                        }
                        match engine.restart_region() {
                            Ok(()) => {
                                reached_eof = false;
                                worker_finished.store(false, Ordering::Release);
                            },
                            Err(playback_error) => {
                                if engine.control.claim_failure() {
                                    let _ = event_tx.send(PlaybackEvent::Failed);
                                    *worker_error.lock().expect("playback error mutex poisoned") =
                                        Some(playback_error);
                                }
                                break;
                            },
                        }
                        continue;
                    }
                    if engine.mode == PlaybackMode::LoopCurrent {
                        if engine.has_cached_cycle() {
                            worker_finished.store(false, Ordering::Release);
                            if !engine.replay_cached_cycle() {
                                thread::yield_now();
                            }
                            continue;
                        }
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
                    match command_rx.recv_timeout(Duration::from_millis(10)) {
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
                    Ok(true) if engine.is_buffer_full() => {
                        // Brief sleep when the ring buffer is full instead of
                        // yielding, which can delay refill for an OS-dependent
                        // interval. A 1ms sleep keeps the worker responsive
                        // to both the callback consuming data and incoming
                        // commands.
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    },
                    Ok(true) if engine.is_waiting_for_buffer_discard() => {
                        // Paused or temporarily inactive output may not call
                        // the consumer immediately. Avoid hot-spinning while
                        // retaining command responsiveness.
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    },
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

    /// Activates an A–B region on the worker thread, or deactivates it when
    /// `None` is passed.
    ///
    /// Activating a region positions the decoder at its start and loops the
    /// selected region without advancing to another file. Deactivating
    /// resumes the selected end-of-file mode from the current position.
    pub fn set_loop_region(&self, region: Option<LoopRegion>) -> Result<(), PlaybackError> {
        let (response_tx, response_rx) = mpsc::sync_channel(1);
        self.commands
            .send(WorkerCommand::SetLoopRegion { region, response: response_tx })
            .map_err(|_| PlaybackError { kind: PlaybackErrorKind::WorkerStopped })?;
        response_rx.recv().map_err(|_| PlaybackError { kind: PlaybackErrorKind::WorkerStopped })?
    }

    /// Deactivates the active A–B region without repositioning playback.
    pub fn clear_loop_region(&self) -> Result<(), PlaybackError> {
        self.set_loop_region(None)
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
