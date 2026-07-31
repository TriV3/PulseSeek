use std::fmt;

use pulseseek_domain::decoder::DecodeError;

/// Error produced by playback operations.
#[derive(Debug)]
pub struct PlaybackError {
    pub(crate) kind: PlaybackErrorKind,
}

#[derive(Debug)]
pub(crate) enum PlaybackErrorKind {
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
