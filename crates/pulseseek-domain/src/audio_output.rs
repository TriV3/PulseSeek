use std::error::Error;
use std::fmt;

use crate::error::{DiagnosticContext, ErrorCategory, ErrorContract, UserErrorDescriptor};
use crate::playback::volume::Volume;

/// Stable identifier for an audio output device.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DeviceId(String);

impl DeviceId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Information about an available audio output device.
#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub id: DeviceId,
    pub name: String,
    /// Maximum number of output channels supported.
    pub max_channels: u16,
    /// Supported sample rates (Hz), empty if unknown.
    pub sample_rates: Vec<u32>,
}

/// State of the output stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamState {
    Playing,
    Paused,
    Stopped,
}

/// Error produced by audio output operations.
#[derive(Debug)]
pub struct AudioOutputError {
    context: DiagnosticContext,
    source: Box<dyn Error + Send + Sync + 'static>,
}

impl AudioOutputError {
    pub fn new<E>(context: DiagnosticContext, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self { context, source: Box::new(source) }
    }
}

impl fmt::Display for AudioOutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "audio output error: {}", self.source)
    }
}

impl Error for AudioOutputError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

impl ErrorContract for AudioOutputError {
    fn user_descriptor(&self) -> UserErrorDescriptor {
        UserErrorDescriptor::for_category(ErrorCategory::Unavailable)
    }

    fn diagnostic_context(&self) -> DiagnosticContext {
        self.context
    }
}

/// Port for audio output hardware control.
///
/// Implementations abstract over platform-specific audio APIs (cpal, etc.)
/// and must be `Send` to allow use across thread boundaries.
pub trait AudioOutput: Send {
    /// List available output devices.
    fn list_devices(&self) -> Result<Vec<DeviceInfo>, AudioOutputError>;

    /// Open and select an output device.
    fn open(&mut self, device: &DeviceId) -> Result<(), AudioOutputError>;

    /// Start or resume playback.
    fn play(&mut self) -> Result<(), AudioOutputError>;

    /// Pause playback.
    fn pause(&mut self) -> Result<(), AudioOutputError>;

    /// Stop playback and release device resources.
    fn stop(&mut self) -> Result<(), AudioOutputError>;

    /// Set the output volume.
    fn set_volume(&mut self, volume: Volume) -> Result<(), AudioOutputError>;

    /// Returns `true` if the current device has been disconnected.
    fn is_device_lost(&self) -> bool;

    /// Returns the currently selected device, if any.
    fn current_device(&self) -> Option<DeviceId>;

    /// Returns the current stream state.
    fn state(&self) -> StreamState;
}
