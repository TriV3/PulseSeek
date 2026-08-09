use std::error::Error;
use std::fmt;

/// Built-in visualization selected for the player workspace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisualizationMode {
    Waveform,
    Logarithmic,
    Linear,
    Musical,
}

impl VisualizationMode {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Waveform => "waveform",
            Self::Logarithmic => "logarithmic",
            Self::Linear => "linear",
            Self::Musical => "musical",
        }
    }

    pub fn from_id(value: &str) -> Option<Self> {
        match value {
            "waveform" => Some(Self::Waveform),
            "logarithmic" => Some(Self::Logarithmic),
            "linear" => Some(Self::Linear),
            "musical" => Some(Self::Musical),
            _ => None,
        }
    }
}

/// Refresh-rate policy for optional real-time visualization work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisualizationQuality {
    Low,
    Balanced,
    High,
}

impl VisualizationQuality {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Balanced => "balanced",
            Self::High => "high",
        }
    }

    pub fn from_id(value: &str) -> Option<Self> {
        match value {
            "low" => Some(Self::Low),
            "balanced" => Some(Self::Balanced),
            "high" => Some(Self::High),
            _ => None,
        }
    }

    pub const fn target_fps(self) -> u32 {
        match self {
            Self::Low => 15,
            Self::Balanced => 30,
            Self::High => 60,
        }
    }
}

/// Persisted built-in visualization preferences, independent of adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VisualizationSettings {
    pub enabled: bool,
    pub mode: VisualizationMode,
    pub quality: VisualizationQuality,
}

impl VisualizationSettings {
    pub const fn new(
        enabled: bool,
        mode: VisualizationMode,
        quality: VisualizationQuality,
    ) -> Self {
        Self { enabled, mode, quality }
    }
}

impl Default for VisualizationSettings {
    fn default() -> Self {
        Self::new(true, VisualizationMode::Waveform, VisualizationQuality::Balanced)
    }
}

/// Maximum interleaved sample count carried by one callback-safe frame.
pub const MAX_VISUALIZATION_FRAME_SAMPLES: usize = 8_192;

/// Immutable time-domain audio captured for off-thread visualization work.
///
/// Samples use fixed storage so constructing and moving a frame never allocates.
#[derive(Clone, Debug, PartialEq)]
pub struct VisualizationFrame {
    sequence: u64,
    position_frames: u64,
    sample_rate: u32,
    channels: u16,
    sample_count: usize,
    samples: [f32; MAX_VISUALIZATION_FRAME_SAMPLES],
}

impl VisualizationFrame {
    pub fn new(
        sequence: u64,
        position_frames: u64,
        sample_rate: u32,
        channels: u16,
        samples: &[f32],
    ) -> Result<Self, VisualizationFrameError> {
        Self::validate_layout(sample_rate, channels, samples.len())?;
        let mut storage = [0.0; MAX_VISUALIZATION_FRAME_SAMPLES];
        storage[..samples.len()].copy_from_slice(samples);
        Ok(Self {
            sequence,
            position_frames,
            sample_rate,
            channels,
            sample_count: samples.len(),
            samples: storage,
        })
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn position_frames(&self) -> u64 {
        self.position_frames
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn samples(&self) -> &[f32] {
        &self.samples[..self.sample_count]
    }

    pub fn validate_layout(
        sample_rate: u32,
        channels: u16,
        sample_count: usize,
    ) -> Result<(), VisualizationFrameError> {
        if sample_rate == 0 {
            return Err(VisualizationFrameError::InvalidSampleRate);
        }
        if channels == 0 {
            return Err(VisualizationFrameError::InvalidChannelCount);
        }
        if sample_count == 0 {
            return Err(VisualizationFrameError::EmptyFrame);
        }
        if sample_count > MAX_VISUALIZATION_FRAME_SAMPLES {
            return Err(VisualizationFrameError::FrameTooLarge {
                sample_count,
                maximum: MAX_VISUALIZATION_FRAME_SAMPLES,
            });
        }
        if !sample_count.is_multiple_of(usize::from(channels)) {
            return Err(VisualizationFrameError::IncompleteInterleavedFrame);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisualizationFrameError {
    InvalidSampleRate,
    InvalidChannelCount,
    EmptyFrame,
    FrameTooLarge { sample_count: usize, maximum: usize },
    IncompleteInterleavedFrame,
}

impl fmt::Display for VisualizationFrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSampleRate => formatter.write_str("sample rate must be greater than zero"),
            Self::InvalidChannelCount => {
                formatter.write_str("channel count must be greater than zero")
            },
            Self::EmptyFrame => formatter.write_str("visualization frame must contain samples"),
            Self::FrameTooLarge { sample_count, maximum } => write!(
                formatter,
                "visualization frame contains {sample_count} samples but the maximum is {maximum}"
            ),
            Self::IncompleteInterleavedFrame => {
                formatter.write_str("sample count must contain complete interleaved frames")
            },
        }
    }
}

impl Error for VisualizationFrameError {}

/// Immutable single-sided magnitude spectrum computed from one visualization frame.
#[derive(Clone, Debug, PartialEq)]
pub struct SpectrumFrame {
    sequence: u64,
    position_frames: u64,
    sample_rate: u32,
    fft_size: usize,
    magnitudes: Box<[f32]>,
}

impl SpectrumFrame {
    pub fn new(
        sequence: u64,
        position_frames: u64,
        sample_rate: u32,
        fft_size: usize,
        magnitudes: Vec<f32>,
    ) -> Result<Self, SpectrumFrameError> {
        if sample_rate == 0 {
            return Err(SpectrumFrameError::InvalidSampleRate);
        }
        if fft_size < 2 || !fft_size.is_power_of_two() {
            return Err(SpectrumFrameError::InvalidFftSize);
        }
        let expected = fft_size / 2 + 1;
        if magnitudes.len() != expected {
            return Err(SpectrumFrameError::InvalidBinCount { actual: magnitudes.len(), expected });
        }
        if magnitudes.iter().any(|magnitude| !magnitude.is_finite() || *magnitude < 0.0) {
            return Err(SpectrumFrameError::InvalidMagnitude);
        }
        Ok(Self {
            sequence,
            position_frames,
            sample_rate,
            fft_size,
            magnitudes: magnitudes.into_boxed_slice(),
        })
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn position_frames(&self) -> u64 {
        self.position_frames
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn fft_size(&self) -> usize {
        self.fft_size
    }

    pub fn bin_width_hz(&self) -> f32 {
        self.sample_rate as f32 / self.fft_size as f32
    }

    pub fn bin_frequency_hz(&self, index: usize) -> Option<f32> {
        (index < self.magnitudes.len()).then(|| index as f32 * self.bin_width_hz())
    }

    pub fn magnitudes(&self) -> &[f32] {
        &self.magnitudes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpectrumFrameError {
    InvalidSampleRate,
    InvalidFftSize,
    InvalidBinCount { actual: usize, expected: usize },
    InvalidMagnitude,
}

impl fmt::Display for SpectrumFrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSampleRate => formatter.write_str("sample rate must be greater than zero"),
            Self::InvalidFftSize => {
                formatter.write_str("FFT size must be a power of two greater than one")
            },
            Self::InvalidBinCount { actual, expected } => {
                write!(formatter, "spectrum contains {actual} bins but expected {expected}")
            },
            Self::InvalidMagnitude => {
                formatter.write_str("spectrum magnitudes must be finite and non-negative")
            },
        }
    }
}

impl Error for SpectrumFrameError {}

/// Energy associated with one equal-tempered musical pitch band.
#[derive(Clone, Debug, PartialEq)]
pub struct MusicalBand {
    note_number: i16,
    lower_frequency_hz: f32,
    center_frequency_hz: f32,
    upper_frequency_hz: f32,
    magnitude: f32,
}

impl MusicalBand {
    pub fn new(
        note_number: i16,
        lower_frequency_hz: f32,
        center_frequency_hz: f32,
        upper_frequency_hz: f32,
        magnitude: f32,
    ) -> Result<Self, MusicalSpectrumFrameError> {
        if !lower_frequency_hz.is_finite()
            || !center_frequency_hz.is_finite()
            || !upper_frequency_hz.is_finite()
            || lower_frequency_hz <= 0.0
            || lower_frequency_hz >= center_frequency_hz
            || center_frequency_hz >= upper_frequency_hz
        {
            return Err(MusicalSpectrumFrameError::InvalidBandFrequencies);
        }
        if !magnitude.is_finite() || magnitude < 0.0 {
            return Err(MusicalSpectrumFrameError::InvalidMagnitude);
        }
        Ok(Self {
            note_number,
            lower_frequency_hz,
            center_frequency_hz,
            upper_frequency_hz,
            magnitude,
        })
    }

    pub fn note_number(&self) -> i16 {
        self.note_number
    }

    pub fn lower_frequency_hz(&self) -> f32 {
        self.lower_frequency_hz
    }

    pub fn center_frequency_hz(&self) -> f32 {
        self.center_frequency_hz
    }

    pub fn upper_frequency_hz(&self) -> f32 {
        self.upper_frequency_hz
    }

    pub fn magnitude(&self) -> f32 {
        self.magnitude
    }
}

/// Immutable pitch-oriented spectrum derived from one FFT spectrum frame.
#[derive(Clone, Debug, PartialEq)]
pub struct MusicalSpectrumFrame {
    sequence: u64,
    position_frames: u64,
    sample_rate: u32,
    tuning_reference_hz: f32,
    bands: Box<[MusicalBand]>,
}

impl MusicalSpectrumFrame {
    pub fn new(
        sequence: u64,
        position_frames: u64,
        sample_rate: u32,
        tuning_reference_hz: f32,
        bands: Vec<MusicalBand>,
    ) -> Result<Self, MusicalSpectrumFrameError> {
        if sample_rate == 0 {
            return Err(MusicalSpectrumFrameError::InvalidSampleRate);
        }
        if !tuning_reference_hz.is_finite() || tuning_reference_hz <= 0.0 {
            return Err(MusicalSpectrumFrameError::InvalidTuningReference);
        }
        if bands.is_empty() {
            return Err(MusicalSpectrumFrameError::EmptyBands);
        }
        if bands.windows(2).any(|pair| {
            pair[0].note_number.checked_add(1) != Some(pair[1].note_number)
                || (pair[0].upper_frequency_hz - pair[1].lower_frequency_hz).abs() > 0.01
        }) {
            return Err(MusicalSpectrumFrameError::NonContiguousBands);
        }
        Ok(Self {
            sequence,
            position_frames,
            sample_rate,
            tuning_reference_hz,
            bands: bands.into_boxed_slice(),
        })
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn position_frames(&self) -> u64 {
        self.position_frames
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn tuning_reference_hz(&self) -> f32 {
        self.tuning_reference_hz
    }

    pub fn bands(&self) -> &[MusicalBand] {
        &self.bands
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MusicalSpectrumFrameError {
    InvalidSampleRate,
    InvalidTuningReference,
    EmptyBands,
    InvalidBandFrequencies,
    InvalidMagnitude,
    NonContiguousBands,
}

impl fmt::Display for MusicalSpectrumFrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSampleRate => formatter.write_str("sample rate must be greater than zero"),
            Self::InvalidTuningReference => {
                formatter.write_str("tuning reference must be finite and greater than zero")
            },
            Self::EmptyBands => formatter.write_str("musical spectrum must contain bands"),
            Self::InvalidBandFrequencies => {
                formatter.write_str("musical band frequencies must be finite and ordered")
            },
            Self::InvalidMagnitude => {
                formatter.write_str("musical band magnitude must be finite and non-negative")
            },
            Self::NonContiguousBands => {
                formatter.write_str("musical spectrum bands must be ordered and contiguous")
            },
        }
    }
}

impl Error for MusicalSpectrumFrameError {}
