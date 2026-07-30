use pulseseek_domain::error::{ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory};
use pulseseek_domain::playback::mode::PlaybackMode;

/// Application service for controlling audio playback.
///
/// This trait abstracts the low-level playback engine, decoder, and audio
/// output behind a narrow command interface. No concrete adapter is exposed
/// across the Tauri boundary.
pub trait PlaybackService: Send {
    /// Starts playback of the file at the given path.
    fn play(&mut self, path: &str) -> Result<(), ApplicationError>;

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
}

/// Fake implementation of [`PlaybackService`] for use in command-envelope tests.
///
/// Records the number of calls per method and returns configurable errors.
pub struct FakePlaybackService {
    pub play_call_count: u64,
    pub pause_call_count: u64,
    pub resume_call_count: u64,
    pub stop_call_count: u64,
    pub seek_call_count: u64,
    pub set_volume_call_count: u64,
    pub last_play_path: Option<String>,
    pub last_seek_position: Option<u64>,
    pub last_volume_gain: Option<f64>,
    pub last_volume_muted: Option<bool>,
    /// When `Some`, all mutating methods return an error with this category.
    pub fail_with: Option<ErrorCategory>,
    /// When set, seek returns this position.
    pub seek_result: Option<u64>,
    pub mode: PlaybackMode,
}

impl FakePlaybackService {
    pub fn new() -> Self {
        Self {
            play_call_count: 0,
            pause_call_count: 0,
            resume_call_count: 0,
            stop_call_count: 0,
            seek_call_count: 0,
            set_volume_call_count: 0,
            last_play_path: None,
            last_seek_position: None,
            last_volume_gain: None,
            last_volume_muted: None,
            fail_with: None,
            seek_result: None,
            mode: PlaybackMode::OneShot,
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
}

impl PlaybackService for FakePlaybackService {
    fn play(&mut self, path: &str) -> Result<(), ApplicationError> {
        self.play_call_count += 1;
        self.last_play_path = Some(path.to_string());
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
}

impl Default for FakePlaybackService {
    fn default() -> Self {
        Self::new()
    }
}
