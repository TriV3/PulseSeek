use std::error::Error;
use std::fmt;

use crate::error::{DiagnosticContext, ErrorCategory, ErrorContract, UserErrorDescriptor};
use crate::playback::position::{Duration, Position, SeekTarget};

/// Result of probing whether a decoder supports given content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeResult {
    Supported,
    NotSupported,
}

/// Metadata describing a decoded audio stream.
#[derive(Clone, Debug, PartialEq)]
pub struct StreamMetadata {
    pub sample_rate: u32,
    pub channels: u16,
    pub duration: Duration,
    /// Bits per sample, `None` for lossy formats (e.g., MP3).
    pub bit_depth: Option<u32>,
    /// Human-readable codec name (e.g., "PCM", "FLAC", "MP3").
    pub codec: &'static str,
}

/// Error produced by decoder operations.
#[derive(Debug)]
pub struct DecodeError {
    context: DiagnosticContext,
    source: Box<dyn Error + Send + Sync + 'static>,
}

impl DecodeError {
    pub fn new<E>(context: DiagnosticContext, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self { context, source: Box::new(source) }
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "decoding failed: {}", self.source)
    }
}

impl Error for DecodeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

impl ErrorContract for DecodeError {
    fn user_descriptor(&self) -> UserErrorDescriptor {
        UserErrorDescriptor::for_category(ErrorCategory::Unavailable)
    }

    fn diagnostic_context(&self) -> DiagnosticContext {
        self.context
    }
}

/// Port for decoding audio content.
///
/// Implementations must be `Send` to allow use across thread boundaries.
pub trait Decoder: Send {
    /// Probe whether this decoder can handle the given content.
    fn probe(&self) -> ProbeResult;

    /// Read stream metadata.
    fn metadata(&mut self) -> Result<StreamMetadata, DecodeError>;

    /// Read up to `buf.len()` frames of decoded PCM data into `buf`.
    ///
    /// Returns the number of frames actually written, which may be less
    /// than `buf.len()` when the end of stream is reached.
    fn read(&mut self, buf: &mut [f32]) -> Result<usize, DecodeError>;

    /// Seek to a validated target position.
    fn seek(&mut self, target: SeekTarget) -> Result<Position, DecodeError>;
}
