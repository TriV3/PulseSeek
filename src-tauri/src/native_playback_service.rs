use std::sync::{Arc, Mutex};

use pulseseek_audio_cpal::CpalAudioOutput;
use pulseseek_domain::audio_output::{AudioOutput, DeviceId};
use pulseseek_domain::decoder::StreamMetadata;
use pulseseek_domain::error::{ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory};
use pulseseek_domain::playback::mode::PlaybackMode;
use pulseseek_domain::playback::position::Position;
use pulseseek_playback::{PlaybackControl, PlaybackWorker};

use crate::playback_events::{NoopEventEmitter, PlaybackEventEmitter};
use crate::playback_service::PlaybackService;

pub struct NativePlaybackService {
    output: Arc<Mutex<CpalAudioOutput>>,
    worker: Option<PlaybackWorker>,
    control: Option<PlaybackControl>,
    events: Arc<dyn PlaybackEventEmitter>,
    current_path: Option<String>,
    current_metadata: Option<StreamMetadata>,
    mode: PlaybackMode,
    buffer_frames: usize,
}

impl NativePlaybackService {
    pub fn new(output: Arc<Mutex<CpalAudioOutput>>) -> Self {
        Self {
            output,
            worker: None,
            control: None,
            events: Arc::new(NoopEventEmitter),
            current_path: None,
            current_metadata: None,
            mode: PlaybackMode::OneShot,
            // Ring buffer capacity. Combined with the 5ms sleep in the
            // worker loop (instead of thread::yield_now), this buffer gives
            // smooth playback without underruns across long files.
            buffer_frames: 131_072,
        }
    }

    fn unavailable(message: &str) -> ApplicationError {
        ApplicationError::new(
            ErrorCategory::Unavailable,
            DiagnosticContext::new(DiagnosticCode::AudioOutput),
            std::io::Error::other(message),
        )
    }
}

impl PlaybackService for NativePlaybackService {
    fn set_events(&mut self, events: Option<Arc<dyn PlaybackEventEmitter>>) {
        if let Some(emitter) = events {
            self.events = emitter;
        }
    }
    fn play(&mut self, path: &str) -> Result<(), ApplicationError> {
        let mut decoder = pulseseek_decoder_symphonia::registry::DecoderRegistry::open(path)
            .map_err(|e| {
                ApplicationError::new(
                    ErrorCategory::InvalidInput,
                    DiagnosticContext::new(DiagnosticCode::BrowserRead),
                    e,
                )
            })?;

        let mut metadata = decoder.metadata().map_err(|e| {
            ApplicationError::new(
                ErrorCategory::Unavailable,
                DiagnosticContext::new(DiagnosticCode::AudioOutput),
                e,
            )
        })?;

        if metadata.sample_rate == 0 {
            metadata.sample_rate = 44_100;
        }
        if metadata.channels == 0 {
            metadata.channels = 2;
        }

        let channels = metadata.channels as usize;
        let sample_rate = metadata.sample_rate;

        let mut output =
            self.output.lock().map_err(|_| Self::unavailable("output lock poisoned"))?;

        if output.is_device_lost() {
            return Err(Self::unavailable(
                "output device was lost; select a device before playing",
            ));
        }

        if output.current_device().is_none() {
            if let Err(e) = output.open(&DeviceId::new("default")) {
                return Err(Self::unavailable(&format!("no output device available: {e}")));
            }
        }

        drop(output);

        // Use the source sample rate directly to avoid resampling. The
        // output stream will use this rate; CoreAudio handles conversion.
        let (worker, consumer) = PlaybackWorker::start(decoder, self.buffer_frames);

        if let Err(mode_result) = worker.set_mode(self.mode) {
            drop(worker);
            drop(consumer);
            let mut output =
                self.output.lock().map_err(|_| Self::unavailable("output lock poisoned"))?;
            let _ = output.stop();
            let _ = self.events.emit_state("failed");
            return Err(Self::unavailable(&format!("failed to set playback mode: {mode_result}")));
        }

        let control = consumer.control();

        let mut output =
            self.output.lock().map_err(|_| Self::unavailable("output lock poisoned"))?;
        output
            .open_stream(consumer, channels, sample_rate)
            .map_err(|e| Self::unavailable(&format!("failed to open output stream: {e}")))?;
        output.play().map_err(|e| Self::unavailable(&format!("failed to start playback: {e}")))?;
        drop(output);

        self.worker = Some(worker);
        self.control = Some(control);
        self.current_path = Some(path.to_string());
        self.current_metadata = Some(metadata);

        let _ = self.events.emit_state("playing");
        Ok(())
    }

    fn pause(&mut self) -> Result<(), ApplicationError> {
        {
            let output =
                self.output.lock().map_err(|_| Self::unavailable("output lock poisoned"))?;
            if output.is_device_lost() {
                return Err(Self::unavailable("output device was lost"));
            }
            if let Some(ref control) = self.control {
                control.pause();
            }
        }
        let mut output =
            self.output.lock().map_err(|_| Self::unavailable("output lock poisoned"))?;
        output.pause().map_err(|e| Self::unavailable(&format!("pause failed: {e}")))?;
        let _ = self.events.emit_state("paused");
        Ok(())
    }

    fn resume(&mut self) -> Result<(), ApplicationError> {
        let mut output =
            self.output.lock().map_err(|_| Self::unavailable("output lock poisoned"))?;
        if output.is_device_lost() {
            return Err(Self::unavailable("output device was lost"));
        }
        if let Some(ref control) = self.control {
            control.resume();
        }
        output.play().map_err(|e| Self::unavailable(&format!("resume failed: {e}")))?;
        let _ = self.events.emit_state("playing");
        Ok(())
    }

    fn stop(&mut self) -> Result<(), ApplicationError> {
        if let Some(ref control) = self.control {
            control.stop();
        }
        // Stop output stream before dropping the worker (which joins the
        // decoder thread).
        let mut output =
            self.output.lock().map_err(|_| Self::unavailable("output lock poisoned"))?;
        let _ = output.stop();
        drop(output);
        self.worker = None;
        self.control = None;
        self.current_path = None;
        self.current_metadata = None;
        let _ = self.events.emit_state("stopped");
        Ok(())
    }

    fn seek(&mut self, position_ms: u64) -> Result<u64, ApplicationError> {
        {
            let output =
                self.output.lock().map_err(|_| Self::unavailable("output lock poisoned"))?;
            if output.is_device_lost() {
                return Err(Self::unavailable("output device was lost"));
            }
        }
        let worker = self.worker.as_ref().ok_or_else(|| Self::unavailable("no active playback"))?;
        let target = pulseseek_domain::playback::position::Duration::Unknown
            .seek_to(Position::from_millis(position_ms))
            .map_err(|_| Self::unavailable("seek position out of bounds"))?;
        let actual =
            worker.seek(target).map_err(|e| Self::unavailable(&format!("seek failed: {e}")))?;
        Ok(actual.as_millis())
    }

    fn set_volume(&mut self, gain: f64, muted: bool) -> Result<(), ApplicationError> {
        let volume = if muted {
            pulseseek_domain::playback::volume::Volume::muted()
        } else {
            let g = pulseseek_domain::playback::volume::Gain::new(gain);
            pulseseek_domain::playback::volume::Volume::new(g)
        };
        let mut output =
            self.output.lock().map_err(|_| Self::unavailable("output lock poisoned"))?;
        output.set_volume(volume).map_err(|e| Self::unavailable(&format!("volume failed: {e}")))?;
        Ok(())
    }

    fn set_mode(&mut self, mode: PlaybackMode) -> Result<PlaybackMode, ApplicationError> {
        self.mode = mode;
        if let Some(ref worker) = self.worker {
            worker
                .set_mode(mode)
                .map_err(|e| Self::unavailable(&format!("set mode failed: {e}")))?;
        }
        Ok(mode)
    }
}
