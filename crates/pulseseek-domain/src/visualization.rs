use std::error::Error;
use std::fmt;

/// Maximum interleaved sample count carried by one callback-safe frame.
pub const MAX_VISUALIZATION_FRAME_SAMPLES: usize = 2_048;

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
