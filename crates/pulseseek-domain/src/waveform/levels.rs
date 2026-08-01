use std::fmt;

use crate::waveform::peak::Peak;

/// Maximum number of resolution levels in one waveform.
pub const MAX_LEVELS: u32 = 8;

/// Validated index into a multiresolution waveform level set.
///
/// Level `0` is the coarsest resolution; higher indices are progressively
/// finer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct LevelIndex(u32);

impl LevelIndex {
    /// Creates a level index, rejecting values at or beyond [`MAX_LEVELS`].
    pub fn new(value: u32) -> Result<Self, LevelIndexError> {
        if value < MAX_LEVELS {
            Ok(Self(value))
        } else {
            Err(LevelIndexError { value })
        }
    }

    /// Numeric value of the level index.
    pub const fn value(self) -> u32 {
        self.0
    }
}

/// Error returned when a level index is outside the valid range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LevelIndexError {
    /// The rejected index value.
    pub value: u32,
}

impl fmt::Display for LevelIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "level index {} exceeds max {}", self.value, MAX_LEVELS - 1)
    }
}

impl std::error::Error for LevelIndexError {}

/// One resolution level of waveform peak data.
#[derive(Clone, Debug, PartialEq)]
pub struct Level {
    /// Position of this level in the resolution pyramid.
    pub index: LevelIndex,
    /// Samples covered by one peak bucket at this level. Coarser levels have
    /// larger values than finer levels.
    pub samples_per_peak: u64,
    /// Peak buckets for the full duration at this resolution.
    pub peaks: Vec<Peak>,
}

/// Multiresolution peak data for one audio item.
///
/// Levels are ordered from coarsest to finest and their indices must be
/// contiguous starting at zero.
#[derive(Clone, Debug, PartialEq)]
pub struct MultiresolutionWaveform {
    levels: Vec<Level>,
}

/// Construction error for a multiresolution waveform.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WaveformError {
    /// A waveform needs at least one level.
    Empty,
    /// Level indices must be contiguous starting at zero.
    NonContiguousIndices,
    /// Every level must contain at least one peak bucket.
    ZeroPeaks,
    /// Every level must have a positive samples-per-peak resolution.
    NonPositiveSamplesPerPeak,
    /// Coarser levels must cover more samples per peak than finer levels.
    NonDecreasingResolution,
}

impl fmt::Display for WaveformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "waveform requires at least one level"),
            Self::NonContiguousIndices => {
                write!(f, "waveform level indices must be contiguous from zero")
            },
            Self::ZeroPeaks => write!(f, "waveform level must contain at least one peak"),
            Self::NonPositiveSamplesPerPeak => {
                write!(f, "waveform level resolution must be positive")
            },
            Self::NonDecreasingResolution => {
                write!(f, "waveform levels must be strictly finer at higher indices")
            },
        }
    }
}

impl std::error::Error for WaveformError {}

impl MultiresolutionWaveform {
    /// Validates and stores a resolution pyramid.
    ///
    /// Fails when the level set is empty, indices are not contiguous from
    /// zero, a level has no peaks or a zero resolution, or a finer level does
    /// not strictly increase resolution.
    pub fn from_levels(levels: Vec<Level>) -> Result<Self, WaveformError> {
        if levels.is_empty() {
            return Err(WaveformError::Empty);
        }
        for (position, lvl) in levels.iter().enumerate() {
            if lvl.index.value() as usize != position {
                return Err(WaveformError::NonContiguousIndices);
            }
            if lvl.peaks.is_empty() {
                return Err(WaveformError::ZeroPeaks);
            }
            if lvl.samples_per_peak == 0 {
                return Err(WaveformError::NonPositiveSamplesPerPeak);
            }
            if position > 0 && levels[position - 1].samples_per_peak <= lvl.samples_per_peak {
                return Err(WaveformError::NonDecreasingResolution);
            }
        }
        Ok(Self { levels })
    }

    /// Levels ordered from coarsest to finest.
    pub fn levels(&self) -> &[Level] {
        &self.levels
    }

    /// The coarsest level (index zero).
    pub fn coarsest(&self) -> &Level {
        &self.levels[0]
    }

    /// The finest level (highest index).
    pub fn finest(&self) -> &Level {
        self.levels.last().expect("validated non-empty")
    }

    /// The level at `index`, if present.
    pub fn level(&self, index: LevelIndex) -> Option<&Level> {
        self.levels.get(index.value() as usize)
    }

    /// Number of resolution levels.
    pub fn len(&self) -> usize {
        self.levels.len()
    }

    /// Whether the waveform contains no levels.
    pub fn is_empty(&self) -> bool {
        self.levels.is_empty()
    }

    /// Selects the coarsest level holding at least `target_peaks` buckets.
    ///
    /// Falls back to the finest level when the target exceeds every level.
    pub fn select_level(&self, target_peaks: u64) -> &Level {
        self.levels
            .iter()
            .find(|lvl| lvl.peaks.len() as u64 >= target_peaks)
            .unwrap_or_else(|| self.finest())
    }
}
