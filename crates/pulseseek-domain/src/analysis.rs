use std::{fmt, str::FromStr};

pub const ANALYSIS_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceId(String);

impl SourceId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionId(String);

impl SessionId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceKind {
    Playback,
    Monitor,
    External,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeasurementPoint {
    Source,
    Monitor,
    ExternalApplication,
    SystemMix,
    InputLoopback,
    DawBridge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChannelLayout {
    Mono,
    Stereo,
    MultiChannel(u16),
}

impl ChannelLayout {
    pub const fn channels(self) -> u16 {
        match self {
            Self::Mono => 1,
            Self::Stereo => 2,
            Self::MultiChannel(channels) => channels,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AudioFormat {
    sample_rate: u32,
    layout: ChannelLayout,
}

impl AudioFormat {
    pub fn new(sample_rate: u32, layout: ChannelLayout) -> Result<Self, AnalysisError> {
        if sample_rate == 0 {
            return Err(AnalysisError::InvalidSampleRate);
        }
        if !matches!(layout, ChannelLayout::Mono | ChannelLayout::Stereo) {
            return Err(AnalysisError::UnsupportedChannelLayout { channels: layout.channels() });
        }
        Ok(Self { sample_rate, layout })
    }

    pub fn mono(sample_rate: u32) -> Result<Self, AnalysisError> {
        Self::new(sample_rate, ChannelLayout::Mono)
    }

    pub fn stereo(sample_rate: u32) -> Result<Self, AnalysisError> {
        Self::new(sample_rate, ChannelLayout::Stereo)
    }

    pub const fn sample_rate(self) -> u32 {
        self.sample_rate
    }
    pub const fn channels(self) -> u16 {
        self.layout.channels()
    }
    pub const fn layout(self) -> ChannelLayout {
        self.layout
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnalysisBlock {
    schema_version: u16,
    source_id: SourceId,
    session_id: SessionId,
    source_kind: SourceKind,
    measurement_point: MeasurementPoint,
    format: AudioFormat,
    first_sample: u64,
    frame_count: u64,
    sequence: u64,
    discontinuity: bool,
    interleaved_samples: Vec<f32>,
}

impl AnalysisBlock {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_id: SourceId,
        session_id: SessionId,
        source_kind: SourceKind,
        measurement_point: MeasurementPoint,
        format: AudioFormat,
        first_sample: u64,
        frame_count: u64,
        sequence: u64,
        discontinuity: bool,
        interleaved_samples: Vec<f32>,
    ) -> Result<Self, AnalysisError> {
        let expected = frame_count
            .checked_mul(u64::from(format.channels()))
            .ok_or(AnalysisError::InvalidFrameCount)?;
        if expected != interleaved_samples.len() as u64 {
            return Err(AnalysisError::SampleCountMismatch {
                expected,
                actual: interleaved_samples.len() as u64,
            });
        }
        if interleaved_samples.iter().any(|sample| !sample.is_finite()) {
            return Err(AnalysisError::InvalidSample);
        }
        Ok(Self {
            schema_version: ANALYSIS_SCHEMA_VERSION,
            source_id,
            session_id,
            source_kind,
            measurement_point,
            format,
            first_sample,
            frame_count,
            sequence,
            discontinuity,
            interleaved_samples,
        })
    }

    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }
    pub fn source_id(&self) -> &SourceId {
        &self.source_id
    }
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }
    pub const fn source_kind(&self) -> SourceKind {
        self.source_kind
    }
    pub const fn measurement_point(&self) -> MeasurementPoint {
        self.measurement_point
    }
    pub const fn format(&self) -> AudioFormat {
        self.format
    }
    pub const fn first_sample(&self) -> u64 {
        self.first_sample
    }
    pub const fn frame_count(&self) -> u64 {
        self.frame_count
    }
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
    pub const fn discontinuity(&self) -> bool {
        self.discontinuity
    }
    pub fn interleaved_samples(&self) -> &[f32] {
        &self.interleaved_samples
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnalysisConfig {
    schema_version: u16,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self { schema_version: ANALYSIS_SCHEMA_VERSION }
    }
}

impl AnalysisConfig {
    pub const fn schema_version(self) -> u16 {
        self.schema_version
    }

    pub fn to_versioned_string(self) -> String {
        format!("analysis:{},{}", self.schema_version, ANALYSIS_SCHEMA_VERSION)
    }

    pub fn from_versioned_string(value: &str) -> Result<Self, AnalysisError> {
        let mut parts =
            value.strip_prefix("analysis:").ok_or(AnalysisError::InvalidConfiguration)?.split(',');
        let schema_version = parts
            .next()
            .and_then(|part| part.parse().ok())
            .ok_or(AnalysisError::InvalidConfiguration)?;
        let algorithm_version: u16 = parts
            .next()
            .and_then(|part| part.parse().ok())
            .ok_or(AnalysisError::InvalidConfiguration)?;
        if parts.next().is_some()
            || schema_version != ANALYSIS_SCHEMA_VERSION
            || algorithm_version != ANALYSIS_SCHEMA_VERSION
        {
            return Err(AnalysisError::UnsupportedSchemaVersion { version: schema_version });
        }
        Ok(Self { schema_version })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnalysisError {
    InvalidSampleRate,
    UnsupportedChannelLayout { channels: u16 },
    InvalidFrameCount,
    SampleCountMismatch { expected: u64, actual: u64 },
    InvalidSample,
    InvalidConfiguration,
    UnsupportedSchemaVersion { version: u16 },
}

impl fmt::Display for AnalysisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for AnalysisError {}

impl FromStr for AnalysisConfig {
    type Err = AnalysisError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::from_versioned_string(value)
    }
}
