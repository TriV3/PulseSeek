use std::fmt;

use super::position::{Duration, Position};

/// Validated A–B loop region.
///
/// A [`LoopRegion`] can only be obtained through [`LoopRegion::new`], which
/// rejects zero-length, reversed, and out-of-bounds points. This guarantees
/// that invalid regions cannot reach the audio engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoopRegion {
    start: Position,
    end: Position,
}

impl LoopRegion {
    /// Validates an A–B region against a duration and returns it on success.
    ///
    /// Rejects equal or reversed points (`start >= end`), points beyond a
    /// known duration, and any region when the duration is unknown.
    pub fn new(
        start: Position,
        end: Position,
        duration: Duration,
    ) -> Result<Self, LoopRegionError> {
        if start >= end {
            return Err(LoopRegionError::ZeroLength { start, end });
        }
        let max = match duration {
            Duration::Known(max) => max,
            Duration::Unknown => return Err(LoopRegionError::UnknownDuration),
        };
        if start > max {
            return Err(LoopRegionError::OutOfBounds { position: start, max });
        }
        if end > max {
            return Err(LoopRegionError::OutOfBounds { position: end, max });
        }
        Ok(Self { start, end })
    }

    /// Returns the region start (A) in milliseconds.
    pub const fn start(&self) -> Position {
        self.start
    }

    /// Returns the region end (B) in milliseconds.
    pub const fn end(&self) -> Position {
        self.end
    }

    /// Returns whether a position falls inside the half-open region
    /// `[start, end)`. The start is included, the end is excluded.
    pub fn contains(&self, position: Position) -> bool {
        position >= self.start && position < self.end
    }

    /// Revalidates the region against a new duration.
    ///
    /// Returns `None` when the region no longer fits the duration.
    pub fn revalidate(&self, duration: Duration) -> Option<Self> {
        Self::new(self.start, self.end, duration).ok()
    }
}

/// Error returned when a loop region cannot be validated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoopRegionError {
    /// Start is not strictly before end (equal or reversed points).
    ZeroLength { start: Position, end: Position },
    /// A point exceeds the known duration.
    OutOfBounds { position: Position, max: Position },
    /// The region cannot be validated against an unknown duration.
    UnknownDuration,
}

impl fmt::Display for LoopRegionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLength { start, end } => {
                write!(f, "loop region start {start} must be before end {end}")
            },
            Self::OutOfBounds { position, max } => {
                write!(f, "loop region point {position} exceeds duration of {max}")
            },
            Self::UnknownDuration => {
                write!(f, "loop region cannot be validated without a known duration")
            },
        }
    }
}

impl std::error::Error for LoopRegionError {}

/// Loop-region state: no region, or an active validated region.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoopRegionState {
    None,
    Set(LoopRegion),
}

impl LoopRegionState {
    /// Returns the state without a loop region.
    pub fn clear(self) -> Self {
        Self::None
    }

    /// Revalidates an active region against a new duration.
    ///
    /// Drops the region when the new duration no longer contains it.
    pub fn revalidate(self, duration: Duration) -> Self {
        match self {
            Self::None => Self::None,
            Self::Set(region) => match region.revalidate(duration) {
                Some(region) => Self::Set(region),
                None => Self::None,
            },
        }
    }
}
