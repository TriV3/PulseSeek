/// Linear gain value clamped to [0.0, MAX].
#[derive(Clone, Copy, Debug)]
pub struct Gain(f64);

impl Gain {
    /// Maximum allowed gain value.
    pub const MAX: f64 = 2.0;

    /// Creates a new gain, clamping the value to the valid range.
    /// NaN or negative values become 0.0.
    pub fn new(value: f64) -> Self {
        if value.is_nan() || value < 0.0 {
            Self(0.0)
        } else if value > Self::MAX {
            Self(Self::MAX)
        } else {
            Self(value)
        }
    }

    /// Returns the gain value as f64.
    pub fn as_f64(&self) -> f64 {
        self.0
    }
}

impl PartialEq for Gain {
    fn eq(&self, other: &Self) -> bool {
        (self.0 - other.0).abs() < f64::EPSILON
    }
}

/// Mute state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mute {
    Off,
    On,
}

/// Volume state combining gain and mute.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Volume {
    gain: Gain,
    mute: Mute,
}

impl Volume {
    /// Creates a new unmuted Volume with the given gain.
    pub fn new(gain: Gain) -> Self {
        Self { gain, mute: Mute::Off }
    }

    /// Creates a muted Volume with zero gain.
    pub fn muted() -> Self {
        Self { gain: Gain::new(0.0), mute: Mute::On }
    }

    /// Returns the current gain.
    pub fn gain(&self) -> Gain {
        self.gain
    }

    /// Returns the current mute state.
    pub fn mute(&self) -> Mute {
        self.mute
    }

    /// Returns a new Volume with the gain replaced.
    pub fn with_gain(mut self, gain: Gain) -> Self {
        self.gain = gain;
        self
    }

    /// Returns a new Volume with the mute state replaced.
    pub fn with_mute(mut self, mute: Mute) -> Self {
        self.mute = mute;
        self
    }

    /// Returns the effective gain (0.0 if muted, otherwise gain value).
    pub fn effective_gain(&self) -> f64 {
        match self.mute {
            Mute::On => 0.0,
            Mute::Off => self.gain.as_f64(),
        }
    }
}
