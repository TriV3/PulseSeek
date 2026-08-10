use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration as ThreadDuration;

use pulseseek_audio_cpal::CpalAudioOutput;
use pulseseek_domain::audio_output::{AudioOutput, DeviceId, StreamState};
use pulseseek_domain::decoder::StreamMetadata;
use pulseseek_domain::error::{ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory};
use pulseseek_domain::playback::loop_region::LoopRegion;
use pulseseek_domain::playback::mode::PlaybackMode;
use pulseseek_domain::playback::position::{Duration, Position};
use pulseseek_domain::visualization::{VisualizationMode, VisualizationSettings};
use pulseseek_playback::{PlaybackControl, PlaybackWorker};

use crate::playback_events::{NoopEventEmitter, PlaybackEventEmitter, EVENT_COMPLETED};
use crate::playback_service::PlaybackService;
use crate::visualization_service::VisualizationPipeline;

pub struct NativePlaybackService {
    output: Arc<Mutex<CpalAudioOutput>>,
    worker: Option<PlaybackWorker>,
    control: Option<PlaybackControl>,
    events: Arc<dyn PlaybackEventEmitter>,
    current_path: Option<String>,
    current_metadata: Option<StreamMetadata>,
    mode: PlaybackMode,
    buffer_frames: usize,
    output_sample_rate: Option<u32>,
    position_reporter: Option<PositionReporter>,
    visualization: Option<VisualizationPipeline>,
    visualization_settings: VisualizationSettings,
}

struct PositionReporter {
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl PositionReporter {
    fn start(
        control: PlaybackControl,
        sample_rate: u32,
        duration_ms: Option<u64>,
        events: Arc<dyn PlaybackEventEmitter>,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let reporter_stop = Arc::clone(&stop);
        let join = std::thread::spawn(move || {
            let _ = events.emit_position(0, duration_ms);
            while !reporter_stop.load(Ordering::Acquire) && !control.is_stopped() {
                std::thread::sleep(ThreadDuration::from_millis(50));
                if reporter_stop.load(Ordering::Acquire) {
                    break;
                }
                let position_ms = frames_to_millis(control.position_frames(), sample_rate);
                let _ = events.emit_position(position_ms, duration_ms);
            }
            if !reporter_stop.load(Ordering::Acquire) && control.is_completed() {
                let _ = events.emit(EVENT_COMPLETED, serde_json::json!({}));
                let _ = events.emit_state("stopped");
            }
        });
        Self { stop, join: Some(join) }
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for PositionReporter {
    fn drop(&mut self) {
        self.stop();
    }
}

fn frames_to_millis(frames: u64, sample_rate: u32) -> u64 {
    if sample_rate == 0 {
        return 0;
    }
    frames.saturating_mul(1_000) / u64::from(sample_rate)
}

fn metadata_duration_ms(metadata: &StreamMetadata) -> Option<u64> {
    match metadata.duration {
        pulseseek_domain::playback::position::Duration::Known(position) => {
            Some(position.as_millis())
        },
        pulseseek_domain::playback::position::Duration::Unknown => None,
    }
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
            output_sample_rate: None,
            position_reporter: None,
            visualization: None,
            visualization_settings: VisualizationSettings::default(),
        }
    }

    fn unavailable(message: &str) -> ApplicationError {
        ApplicationError::new(
            ErrorCategory::Unavailable,
            DiagnosticContext::new(DiagnosticCode::AudioOutput),
            std::io::Error::other(message),
        )
    }

    fn invalid_region(message: &str) -> ApplicationError {
        ApplicationError::new(
            ErrorCategory::InvalidInput,
            DiagnosticContext::new(DiagnosticCode::PlaybackControl),
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
        if let Some(mut reporter) = self.position_reporter.take() {
            reporter.stop();
        }
        self.visualization = None;
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

        let output_sample_rate = output
            .output_sample_rate()
            .map_err(|e| Self::unavailable(&format!("failed to read output sample rate: {e}")))?;
        drop(output);

        let (worker, mut consumer) = PlaybackWorker::start_resampled(
            decoder,
            self.buffer_frames,
            channels,
            sample_rate,
            output_sample_rate,
        )
        .map_err(|e| {
            Self::unavailable(&format!("failed to configure sample-rate conversion: {e}"))
        })?;

        if let Err(mode_result) = worker.set_mode(self.mode) {
            drop(worker);
            drop(consumer);
            let mut output =
                self.output.lock().map_err(|_| Self::unavailable("output lock poisoned"))?;
            let _ = output.stop();
            let _ = self.events.emit_state("failed");
            return Err(Self::unavailable(&format!("failed to set playback mode: {mode_result}")));
        }

        let visualization = match VisualizationPipeline::start_with_settings(
            output_sample_rate,
            channels,
            self.visualization_settings,
            Arc::clone(&self.events),
        ) {
            Ok((pipeline, tap)) => {
                consumer.set_visualization_tap(tap);
                Some(pipeline)
            },
            Err(error) => {
                tracing::warn!(error = %error, "visualization pipeline unavailable; playback continues");
                None
            },
        };

        let control = consumer.control();

        let mut output =
            self.output.lock().map_err(|_| Self::unavailable("output lock poisoned"))?;
        output
            .open_stream(consumer, channels)
            .map_err(|e| Self::unavailable(&format!("failed to open output stream: {e}")))?;
        output.play().map_err(|e| Self::unavailable(&format!("failed to start playback: {e}")))?;
        drop(output);

        self.worker = Some(worker);
        self.control = Some(control.clone());
        self.current_path = Some(path.to_string());
        self.current_metadata = Some(metadata);
        self.output_sample_rate = Some(output_sample_rate);
        self.visualization = visualization;

        let duration_ms = self.current_metadata.as_ref().and_then(metadata_duration_ms);
        self.position_reporter = Some(PositionReporter::start(
            control,
            output_sample_rate,
            duration_ms,
            Arc::clone(&self.events),
        ));

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
        if let Some(mut reporter) = self.position_reporter.take() {
            reporter.stop();
        }
        if let Some(ref control) = self.control {
            control.stop();
        }
        // Stop output stream before dropping the worker (which joins the
        // decoder thread).
        let mut output =
            self.output.lock().map_err(|_| Self::unavailable("output lock poisoned"))?;
        let _ = output.stop();
        drop(output);
        self.visualization = None;
        self.worker = None;
        self.control = None;
        self.current_path = None;
        self.current_metadata = None;
        self.output_sample_rate = None;
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
        if let (Some(control), Some(sample_rate)) = (&self.control, self.output_sample_rate) {
            let frames = actual.as_millis().saturating_mul(u64::from(sample_rate)) / 1_000;
            control.set_position_frames(frames);
        }
        let duration_ms = self.current_metadata.as_ref().and_then(metadata_duration_ms);
        let _ = self.events.emit_position(actual.as_millis(), duration_ms);
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

    fn set_visualization_settings(
        &mut self,
        mut settings: VisualizationSettings,
    ) -> Result<(), ApplicationError> {
        settings.enabled = settings.enabled && settings.mode != VisualizationMode::Waveform;
        self.visualization_settings = settings;
        if let Some(pipeline) = &self.visualization {
            pipeline.configure(settings.enabled, settings.quality);
        }
        Ok(())
    }

    fn set_mode(&mut self, mode: PlaybackMode) -> Result<PlaybackMode, ApplicationError> {
        if let Some(ref worker) = self.worker {
            if let Err(error) = worker.set_mode(mode) {
                let playback_has_ended =
                    self.control.as_ref().is_some_and(|control| control.is_stopped());
                if !playback_has_ended {
                    return Err(Self::unavailable(&format!("set mode failed: {error}")));
                }
            }
        }
        self.mode = mode;
        Ok(mode)
    }

    fn set_loop_region(&mut self, start_ms: u64, end_ms: u64) -> Result<u64, ApplicationError> {
        let duration_ms = self
            .current_metadata
            .as_ref()
            .and_then(metadata_duration_ms)
            .ok_or_else(|| Self::invalid_region("no active playback with a known duration"))?;
        let region = LoopRegion::new(
            Position::from_millis(start_ms),
            Position::from_millis(end_ms),
            Duration::from_millis(duration_ms),
        )
        .map_err(|error| Self::invalid_region(&error.to_string()))?;
        let worker = self.worker.as_ref().ok_or_else(|| Self::unavailable("no active playback"))?;
        worker
            .set_loop_region(Some(region))
            .map_err(|error| Self::unavailable(&format!("set loop region failed: {error}")))?;
        let confirmed = region.start().as_millis();
        if let (Some(control), Some(sample_rate)) = (&self.control, self.output_sample_rate) {
            let frames = confirmed.saturating_mul(u64::from(sample_rate)) / 1_000;
            control.set_position_frames(frames);
        }
        let _ = self.events.emit_position(confirmed, Some(duration_ms));
        Ok(confirmed)
    }

    fn clear_loop_region(&mut self) -> Result<(), ApplicationError> {
        if let Some(ref worker) = self.worker {
            worker.clear_loop_region().map_err(|error| {
                Self::unavailable(&format!("clear loop region failed: {error}"))
            })?;
        }
        Ok(())
    }

    fn reconcile_path(&mut self, old_path: &str, new_path: &str) -> Result<bool, ApplicationError> {
        let Some(current) = self.current_path.as_deref() else {
            return Ok(false);
        };
        if current != old_path {
            return Ok(false);
        }
        self.current_path = Some(new_path.to_string());
        Ok(true)
    }

    fn select_output_device(&mut self, device_id: &str) -> Result<(), ApplicationError> {
        let output_is_already_usable = {
            let output =
                self.output.lock().map_err(|_| Self::unavailable("output lock poisoned"))?;
            !output.is_device_lost()
                && output.current_device().is_some_and(|current| current.as_str() == device_id)
        };
        if output_is_already_usable {
            return Ok(());
        }
        let session_is_active = self
            .control
            .as_ref()
            .is_some_and(|control| !control.is_stopped() && !control.is_completed());
        if !session_is_active {
            let mut output =
                self.output.lock().map_err(|_| Self::unavailable("output lock poisoned"))?;
            return output
                .open(&DeviceId::new(device_id))
                .map_err(|error| Self::unavailable(&format!("device change failed: {error}")));
        }

        let path = self
            .current_path
            .clone()
            .ok_or_else(|| Self::unavailable("active playback path unavailable"))?;
        let sample_rate = self.output_sample_rate.unwrap_or(44_100);
        let position_ms = self
            .control
            .as_ref()
            .map(|control| frames_to_millis(control.position_frames(), sample_rate))
            .unwrap_or(0);
        let was_paused =
            self.output.lock().map_err(|_| Self::unavailable("output lock poisoned"))?.state()
                == StreamState::Paused;

        if let Some(mut reporter) = self.position_reporter.take() {
            reporter.stop();
        }
        {
            let mut output =
                self.output.lock().map_err(|_| Self::unavailable("output lock poisoned"))?;
            output
                .open(&DeviceId::new(device_id))
                .map_err(|error| Self::unavailable(&format!("device change failed: {error}")))?;
        }
        self.worker = None;
        self.visualization = None;
        self.control = None;
        self.current_path = None;
        self.current_metadata = None;
        self.output_sample_rate = None;

        self.play(&path)?;
        if position_ms > 0 {
            self.seek(position_ms)?;
        }
        if was_paused {
            self.pause()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use pulseseek_audio_cpal::CpalAudioOutput;
    use pulseseek_domain::error::ErrorContract;

    use super::*;

    fn service() -> NativePlaybackService {
        NativePlaybackService::new(Arc::new(Mutex::new(CpalAudioOutput::new())))
    }

    #[test]
    fn output_frames_are_converted_to_wall_clock_milliseconds() {
        assert_eq!(frames_to_millis(48_000, 48_000), 1_000);
        assert_eq!(frames_to_millis(44_100, 44_100), 1_000);
    }

    #[test]
    fn waveform_settings_never_enable_native_fft_work() {
        let mut service = service();

        service.set_visualization_settings(VisualizationSettings::default()).unwrap();

        assert!(!service.visualization_settings.enabled);
    }

    #[test]
    fn reconcile_path_updates_current_path_when_playing_renamed_file() {
        let mut service = service();
        service.current_path = Some("/music/track.wav".to_string());

        let reconciled = service
            .reconcile_path("/music/track.wav", "/music/renamed.wav")
            .expect("reconcile succeeds");
        assert!(reconciled, "renamed file is the playing file");
        assert_eq!(service.current_path.as_deref(), Some("/music/renamed.wav"));
    }

    #[test]
    fn reconcile_path_ignores_rename_of_other_file() {
        let mut service = service();
        service.current_path = Some("/music/track.wav".to_string());

        let reconciled = service
            .reconcile_path("/music/other.wav", "/music/other-renamed.wav")
            .expect("reconcile succeeds");
        assert!(!reconciled, "rename of another file is not reconciled");
        assert_eq!(service.current_path.as_deref(), Some("/music/track.wav"));
    }

    #[test]
    fn reconcile_path_returns_false_without_active_session() {
        let mut service = service();

        let reconciled = service
            .reconcile_path("/music/track.wav", "/music/renamed.wav")
            .expect("reconcile succeeds");
        assert!(!reconciled, "no active session means no reconciliation");
        assert!(service.current_path.is_none());
    }

    fn metadata_with_duration(ms: u64) -> StreamMetadata {
        StreamMetadata {
            sample_rate: 44_100,
            channels: 2,
            duration: Duration::from_millis(ms),
            bit_depth: Some(16),
            codec: "PCM",
        }
    }

    #[test]
    fn set_loop_region_rejects_reversed_points_before_touching_worker() {
        let mut service = service();
        service.current_metadata = Some(metadata_with_duration(10_000));

        let error = service.set_loop_region(5_000, 2_000).expect_err("reversed region is rejected");
        assert_eq!(error.user_descriptor().category(), ErrorCategory::InvalidInput);
        assert_eq!(error.diagnostic_context().code(), "playback.control");
    }

    #[test]
    fn set_loop_region_rejects_points_beyond_duration() {
        let mut service = service();
        service.current_metadata = Some(metadata_with_duration(10_000));

        let error =
            service.set_loop_region(9_000, 11_000).expect_err("out-of-bounds end is rejected");
        assert_eq!(error.user_descriptor().category(), ErrorCategory::InvalidInput);
    }

    #[test]
    fn set_loop_region_requires_an_active_session() {
        let mut service = service();

        let error =
            service.set_loop_region(1_000, 5_000).expect_err("no session means no duration");
        assert_eq!(error.user_descriptor().category(), ErrorCategory::InvalidInput);
    }

    #[test]
    fn set_loop_region_requires_a_running_worker() {
        let mut service = service();
        service.current_metadata = Some(metadata_with_duration(10_000));

        let error = service.set_loop_region(1_000, 5_000).expect_err("no worker means no engine");
        assert_eq!(error.user_descriptor().category(), ErrorCategory::Unavailable);
    }

    #[test]
    fn clear_loop_region_is_a_noop_without_an_active_worker() {
        let mut service = service();

        service.clear_loop_region().expect("clearing without a worker succeeds");
    }
}
