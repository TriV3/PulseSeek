use std::fmt;

/// Non-negative playback position in milliseconds.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Position(u64);

impl Position {
    pub const fn from_millis(ms: u64) -> Self {
        Self(ms)
    }

    pub const fn as_millis(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}ms", self.0)
    }
}

/// Total duration of a playable item.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Duration {
    Known(Position),
    Unknown,
}

impl Duration {
    pub const fn from_millis(ms: u64) -> Self {
        Duration::Known(Position::from_millis(ms))
    }

    /// Validates seek target against duration bounds.
    ///
    /// Returns `SeekError` if seeking beyond a known finite duration.
    pub fn seek_to(&self, target: Position) -> Result<SeekTarget, SeekError> {
        match self {
            Duration::Known(max) if target > *max => {
                Err(SeekError { requested: target, max: *max })
            },
            _ => Ok(SeekTarget(target)),
        }
    }
}

/// Validated seek position that will not exceed its originating duration.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct SeekTarget(Position);

impl SeekTarget {
    pub const fn position(&self) -> Position {
        self.0
    }
}

/// Error returned when a seek target exceeds the available duration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeekError {
    pub requested: Position,
    pub max: Position,
}

impl fmt::Display for SeekError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "seek to {} exceeds duration of {}", self.requested, self.max)
    }
}

impl std::error::Error for SeekError {}
