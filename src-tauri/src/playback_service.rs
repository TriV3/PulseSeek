use std::sync::Arc;

use pulseseek_domain::error::{ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory};
use pulseseek_domain::playback::loop_region::LoopRegion;
use pulseseek_domain::playback::mode::PlaybackMode;
use pulseseek_domain::playback::position::{Duration, Position};
use pulseseek_domain::visualization::VisualizationSettings;

use crate::playback_events::PlaybackEventEmitter;

/// Application service for controlling audio playback.
///
/// This trait abstracts the low-level playback engine, decoder, and audio
/// output behind a narrow command interface. No concrete adapter is exposed
/// across the Tauri boundary.
pub trait PlaybackService: Send {
    /// Starts playback of the file at the given path.
    fn play(&mut self, path: &str) -> Result<(), ApplicationError>;

    fn prepare_next(&mut self, path: &str) -> Result<(), ApplicationError>;

    fn clear_prepared(&mut self) -> Result<(), ApplicationError>;

    /// Pauses playback without discarding buffered frames.
    fn pause(&mut self) -> Result<(), ApplicationError>;

    /// Resumes playback from the paused position.
    fn resume(&mut self) -> Result<(), ApplicationError>;

    /// Stops playback and resets the position.
    fn stop(&mut self) -> Result<(), ApplicationError>;

    /// Seeks to the given millisecond position.
    ///
    /// Returns the actual position reached after the seek.
    fn seek(&mut self, position_ms: u64) -> Result<u64, ApplicationError>;

    /// Sets the volume gain and mute state.
    fn set_volume(&mut self, gain: f64, muted: bool) -> Result<(), ApplicationError>;

    /// Changes end-of-file playback mode.
    fn set_mode(&mut self, mode: PlaybackMode) -> Result<PlaybackMode, ApplicationError>;

    /// Activates an A–B repeat region and returns the confirmed start
    /// position. Invalid regions (reversed, equal, or out-of-bounds points)
    /// are rejected before they can reach the audio engine.
    fn set_loop_region(&mut self, start_ms: u64, end_ms: u64) -> Result<u64, ApplicationError>;

    /// Deactivates the active A–B repeat region.
    fn clear_loop_region(&mut self) -> Result<(), ApplicationError>;

    /// Applies optional visualization work without restarting playback.
    fn set_visualization_settings(
        &mut self,
        _settings: VisualizationSettings,
    ) -> Result<(), ApplicationError> {
        Ok(())
    }

    /// Reconciles the tracked current path after an external rename.
    ///
    /// Returns `true` when `old_path` is the currently playing file and its
    /// tracked path was updated to `new_path`; `false` when the rename does
    /// not concern the active session (FR-FM-009, FR-FM-010). The already
    /// open decoder keeps streaming the original inode on POSIX, so playback
    /// itself is untouched.
    fn reconcile_path(&mut self, old_path: &str, new_path: &str) -> Result<bool, ApplicationError>;

    /// Rebinds the active playback session to another output device while
    /// preserving its file, position, and playing/paused state.
    fn select_output_device(&mut self, device_id: &str) -> Result<(), ApplicationError>;

    /// Provides a real event emitter to a native service. No-op default for
    /// fake implementations.
    fn set_events(&mut self, _events: Option<Arc<dyn PlaybackEventEmitter>>) {}
}

/// Fake implementation of [`PlaybackService`] for use in command-envelope tests.
///
/// Records the number of calls per method and returns configurable errors.
pub struct FakePlaybackService {
    pub play_call_count: u64,
    pub prepare_next_call_count: u64,
    pub pause_call_count: u64,
    pub resume_call_count: u64,
    pub stop_call_count: u64,
    pub seek_call_count: u64,
    pub set_volume_call_count: u64,
    pub select_output_device_call_count: u64,
    pub set_loop_region_call_count: u64,
    pub clear_loop_region_call_count: u64,
    pub last_play_path: Option<String>,
    pub last_seek_position: Option<u64>,
    pub last_volume_gain: Option<f64>,
    pub last_volume_muted: Option<bool>,
    pub last_output_device_id: Option<String>,
    pub last_loop_region_start: Option<u64>,
    pub last_loop_region_end: Option<u64>,
    /// Number of `reconcile_path` calls.
    pub reconcile_path_call_count: u64,
    /// Last `old_path` passed to `reconcile_path`.
    pub last_reconcile_old_path: Option<String>,
    /// Last `new_path` passed to `reconcile_path`.
    pub last_reconcile_new_path: Option<String>,
    /// When set, `reconcile_path` returns this value instead of `false`.
    pub reconcile_path_result: Option<bool>,
    /// When `Some`, all mutating methods return an error with this category.
    pub fail_with: Option<ErrorCategory>,
    /// When set, seek returns this position.
    pub seek_result: Option<u64>,
    /// Track duration used to validate loop regions. `None` makes the fake
    /// reject every region with an unknown-duration error.
    pub loop_region_duration_ms: Option<u64>,
    pub mode: PlaybackMode,
    pub visualization_settings: VisualizationSettings,
}

impl FakePlaybackService {
    pub fn new() -> Self {
        Self {
            play_call_count: 0,
            prepare_next_call_count: 0,
            pause_call_count: 0,
            resume_call_count: 0,
            stop_call_count: 0,
            seek_call_count: 0,
            set_volume_call_count: 0,
            select_output_device_call_count: 0,
            set_loop_region_call_count: 0,
            clear_loop_region_call_count: 0,
            last_play_path: None,
            last_seek_position: None,
            last_volume_gain: None,
            last_volume_muted: None,
            last_output_device_id: None,
            last_loop_region_start: None,
            last_loop_region_end: None,
            reconcile_path_call_count: 0,
            last_reconcile_old_path: None,
            last_reconcile_new_path: None,
            reconcile_path_result: None,
            fail_with: None,
            seek_result: None,
            loop_region_duration_ms: Some(100_000),
            mode: PlaybackMode::OneShot,
            visualization_settings: VisualizationSettings::default(),
        }
    }

    fn check_fail(&self) -> Result<(), ApplicationError> {
        match self.fail_with {
            Some(category) => Err(ApplicationError::new(
                category,
                DiagnosticContext::new(DiagnosticCode::PlaybackControl),
                std::io::Error::other("fake playback error"),
            )),
            None => Ok(()),
        }
    }

    fn invalid_region(&self, message: &str) -> ApplicationError {
        ApplicationError::new(
            ErrorCategory::InvalidInput,
            DiagnosticContext::new(DiagnosticCode::PlaybackControl),
            std::io::Error::other(message),
        )
    }
}

impl PlaybackService for FakePlaybackService {
    fn play(&mut self, path: &str) -> Result<(), ApplicationError> {
        self.play_call_count += 1;
        self.last_play_path = Some(path.to_string());
        self.check_fail()
    }

    fn prepare_next(&mut self, _path: &str) -> Result<(), ApplicationError> {
        self.prepare_next_call_count += 1;
        self.check_fail()
    }

    fn clear_prepared(&mut self) -> Result<(), ApplicationError> {
        self.check_fail()
    }

    fn pause(&mut self) -> Result<(), ApplicationError> {
        self.pause_call_count += 1;
        self.check_fail()
    }

    fn resume(&mut self) -> Result<(), ApplicationError> {
        self.resume_call_count += 1;
        self.check_fail()
    }

    fn stop(&mut self) -> Result<(), ApplicationError> {
        self.stop_call_count += 1;
        self.check_fail()
    }

    fn seek(&mut self, position_ms: u64) -> Result<u64, ApplicationError> {
        self.seek_call_count += 1;
        self.last_seek_position = Some(position_ms);
        self.check_fail()?;
        Ok(self.seek_result.unwrap_or(position_ms))
    }

    fn set_volume(&mut self, gain: f64, muted: bool) -> Result<(), ApplicationError> {
        self.set_volume_call_count += 1;
        self.last_volume_gain = Some(gain);
        self.last_volume_muted = Some(muted);
        self.check_fail()
    }

    fn set_mode(&mut self, mode: PlaybackMode) -> Result<PlaybackMode, ApplicationError> {
        self.check_fail()?;
        self.mode = mode;
        Ok(self.mode)
    }

    fn set_loop_region(&mut self, start_ms: u64, end_ms: u64) -> Result<u64, ApplicationError> {
        self.set_loop_region_call_count += 1;
        self.last_loop_region_start = Some(start_ms);
        self.last_loop_region_end = Some(end_ms);
        self.check_fail()?;
        let duration = match self.loop_region_duration_ms {
            Some(ms) => Duration::from_millis(ms),
            None => {
                return Err(
                    self.invalid_region("loop region cannot be validated without a known duration")
                );
            },
        };
        let region = LoopRegion::new(
            Position::from_millis(start_ms),
            Position::from_millis(end_ms),
            duration,
        )
        .map_err(|error| self.invalid_region(&error.to_string()))?;
        Ok(region.start().as_millis())
    }

    fn clear_loop_region(&mut self) -> Result<(), ApplicationError> {
        self.clear_loop_region_call_count += 1;
        self.check_fail()
    }

    fn set_visualization_settings(
        &mut self,
        settings: VisualizationSettings,
    ) -> Result<(), ApplicationError> {
        self.visualization_settings = settings;
        self.check_fail()
    }

    fn reconcile_path(&mut self, old_path: &str, new_path: &str) -> Result<bool, ApplicationError> {
        self.reconcile_path_call_count += 1;
        self.last_reconcile_old_path = Some(old_path.to_string());
        self.last_reconcile_new_path = Some(new_path.to_string());
        self.check_fail()?;
        Ok(self.reconcile_path_result.unwrap_or(false))
    }

    fn select_output_device(&mut self, device_id: &str) -> Result<(), ApplicationError> {
        self.select_output_device_call_count += 1;
        self.last_output_device_id = Some(device_id.to_string());
        self.check_fail()
    }
}

impl Default for FakePlaybackService {
    fn default() -> Self {
        Self::new()
    }
}
