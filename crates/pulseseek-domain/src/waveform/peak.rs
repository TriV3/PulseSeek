/// Bounded min/max amplitude pair for one waveform bucket.
///
/// The pair is normalized on construction so that `min <= max` and both
/// bounds lie within `[-1.0, 1.0]`. Non-finite inputs are clamped to `0.0`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Peak {
    min: f32,
    max: f32,
}

impl Peak {
    /// Minimum amplitude a normalized peak bound can carry.
    pub const AMPLITUDE_MIN: f32 = -1.0;

    /// Maximum amplitude a normalized peak bound can carry.
    pub const AMPLITUDE_MAX: f32 = 1.0;

    /// Builds a peak from unordered, possibly out-of-range bounds.
    ///
    /// Both bounds are clamped into `[AMPLITUDE_MIN, AMPLITUDE_MAX]` and the
    /// pair is ordered so that `min <= max`. `NaN` bounds become `0.0`.
    pub fn from_parts(min: f32, max: f32) -> Self {
        let min = clamp_amplitude(min);
        let max = clamp_amplitude(max);
        if min <= max {
            Self { min, max }
        } else {
            Self { min: max, max: min }
        }
    }

    /// Lower amplitude bound of the bucket.
    pub const fn min(self) -> f32 {
        self.min
    }

    /// Upper amplitude bound of the bucket.
    pub const fn max(self) -> f32 {
        self.max
    }
}

fn clamp_amplitude(value: f32) -> f32 {
    if value.is_nan() {
        return 0.0;
    }
    value.clamp(Peak::AMPLITUDE_MIN, Peak::AMPLITUDE_MAX)
}
