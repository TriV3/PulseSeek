use std::fmt;

use crate::playback::position::{Duration, Position};

/// Maps between time positions and pixel coordinates for a waveform rendered
/// at a fixed width over a known duration.
///
/// The mapping is clamped: out-of-range pixels map to the rendered span and
/// out-of-range positions map to the first or last pixel. All arithmetic uses
/// `u128` intermediates so extreme durations or widths cannot overflow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timeline {
    width_px: u64,
    duration_ms: u64,
}

impl Timeline {
    /// Creates a time/pixel mapping.
    ///
    /// Fails when the pixel width is zero or the duration is unknown.
    pub fn new(width_px: u64, duration: Duration) -> Result<Self, TimelineError> {
        if width_px == 0 {
            return Err(TimelineError::ZeroWidth);
        }
        let duration_ms = match duration {
            Duration::Known(position) => position.as_millis(),
            Duration::Unknown => return Err(TimelineError::UnknownDuration),
        };
        Ok(Self { width_px, duration_ms })
    }

    /// Width of the rendered span in pixels.
    pub const fn width_px(self) -> u64 {
        self.width_px
    }

    /// Known duration of the rendered span.
    pub fn duration(self) -> Position {
        Position::from_millis(self.duration_ms)
    }

    /// Maps a pixel coordinate to a time position.
    ///
    /// Pixels before zero clamp to the start; pixels at or beyond the width
    /// clamp to the full duration.
    pub fn position_at(&self, x: i64) -> Position {
        if self.duration_ms == 0 || self.width_px == 1 {
            return Position::from_millis(0);
        }
        let span = self.width_px - 1;
        let x = clamp_pixel(x, span);
        let ms = round_div(x as u128 * self.duration_ms as u128, span as u128);
        Position::from_millis(ms as u64)
    }

    /// Maps a time position to a pixel coordinate.
    ///
    /// Positions at or beyond the duration clamp to the last pixel.
    pub fn pixel_for(&self, position: Position) -> u64 {
        if self.duration_ms == 0 || self.width_px == 1 {
            return 0;
        }
        let span = self.width_px - 1;
        let ms = position.as_millis().min(self.duration_ms);
        let px = round_div(ms as u128 * span as u128, self.duration_ms as u128);
        px as u64
    }
}

fn clamp_pixel(x: i64, span: u64) -> u64 {
    if x < 0 {
        0
    } else {
        (x as u64).min(span)
    }
}

fn round_div(numerator: u128, denominator: u128) -> u128 {
    (numerator + denominator / 2) / denominator
}

/// Error returned when a timeline cannot be constructed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimelineError {
    /// A rendered span needs at least one pixel.
    ZeroWidth,
    /// A rendered span needs a known duration.
    UnknownDuration,
}

impl fmt::Display for TimelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroWidth => write!(f, "timeline width must be positive"),
            Self::UnknownDuration => write!(f, "timeline requires a known duration"),
        }
    }
}

impl std::error::Error for TimelineError {}
