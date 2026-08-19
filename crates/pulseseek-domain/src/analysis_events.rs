use crate::analysis::{MeasurementPoint, SessionId, SourceId};

pub const EVENT_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SchemaVersion(u16);
impl SchemaVersion {
    pub fn new(value: u16) -> Result<Self, EventContractError> {
        if value == EVENT_SCHEMA_VERSION {
            Ok(Self(value))
        } else {
            Err(EventContractError::UnsupportedSchemaVersion(value))
        }
    }
    pub const fn value(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EventFamily {
    Session,
    Levels,
    Spectrum,
    Bands,
    Spectrogram,
    Waveform,
    Loudness,
    TruePeak,
    Stereo,
    Diagnostics,
}
impl EventFamily {
    pub const ALL: [Self; 10] = [
        Self::Session,
        Self::Levels,
        Self::Spectrum,
        Self::Bands,
        Self::Spectrogram,
        Self::Waveform,
        Self::Loudness,
        Self::TruePeak,
        Self::Stereo,
        Self::Diagnostics,
    ];
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Session => "metering.session",
            Self::Levels => "metering.levels",
            Self::Spectrum => "metering.spectrum",
            Self::Bands => "metering.bands",
            Self::Spectrogram => "metering.spectrogram",
            Self::Waveform => "metering.waveform",
            Self::Loudness => "metering.loudness",
            Self::TruePeak => "metering.true_peak",
            Self::Stereo => "metering.stereo",
            Self::Diagnostics => "metering.diagnostics",
        }
    }
    pub const fn policy(self) -> DeliveryPolicy {
        match self {
            Self::Session => DeliveryPolicy::OnChange,
            Self::Spectrogram | Self::Waveform => DeliveryPolicy::LatestOnly,
            Self::Loudness | Self::TruePeak => DeliveryPolicy::ContinuousAndDisplay,
            Self::Diagnostics => DeliveryPolicy::Cadenced { min_hz: 1, max_hz: 5 },
            _ => DeliveryPolicy::Cadenced { min_hz: 15, max_hz: 60 },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryPolicy {
    OnChange,
    LatestOnly,
    ContinuousAndDisplay,
    Cadenced { min_hz: u16, max_hz: u16 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Validity {
    Measured,
    Estimated,
    Stale,
    Interpolated,
    Incomplete,
    Unavailable,
    Invalid,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidReason {
    QueueSaturated,
    SourceUnavailable,
    PermissionDenied,
    UnsupportedFormat,
    Configuration,
    ContinuousGap,
    InvalidInput,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventValidity {
    Measured,
    Estimated,
    Stale,
    Interpolated,
    Incomplete,
    Unavailable,
    Invalid { reason: InvalidReason },
}
impl EventValidity {
    pub const fn validity(self) -> Validity {
        match self {
            Self::Measured => Validity::Measured,
            Self::Estimated => Validity::Estimated,
            Self::Stale => Validity::Stale,
            Self::Interpolated => Validity::Interpolated,
            Self::Incomplete => Validity::Incomplete,
            Self::Unavailable => Validity::Unavailable,
            Self::Invalid { .. } => Validity::Invalid,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventMetadata {
    schema_version: SchemaVersion,
    session_id: SessionId,
    source_id: SourceId,
    source_point: MeasurementPoint,
    sequence: u64,
    timestamp_samples: u64,
    validity: EventValidity,
}
impl EventMetadata {
    pub fn new(
        session_id: SessionId,
        source_id: SourceId,
        source_point: MeasurementPoint,
        sequence: u64,
        timestamp_samples: u64,
        validity: EventValidity,
    ) -> Result<Self, EventContractError> {
        Ok(Self {
            schema_version: SchemaVersion::new(EVENT_SCHEMA_VERSION)?,
            session_id,
            source_id,
            source_point,
            sequence,
            timestamp_samples,
            validity,
        })
    }
    pub fn validate_after(&self, previous: &Self) -> Result<(), EventContractError> {
        if self.session_id != previous.session_id {
            return Ok(());
        }
        if self.sequence <= previous.sequence {
            return Err(EventContractError::SequenceNotIncreasing);
        }
        if self.timestamp_samples < previous.timestamp_samples {
            return Err(EventContractError::TimestampDecreased);
        }
        Ok(())
    }
    pub const fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }
    pub fn source_id(&self) -> &SourceId {
        &self.source_id
    }
    pub const fn source_point(&self) -> MeasurementPoint {
        self.source_point
    }
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
    pub const fn timestamp_samples(&self) -> u64 {
        self.timestamp_samples
    }
    pub const fn validity(&self) -> EventValidity {
        self.validity
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExperimentalMetadata {
    formula: String,
    baseline: String,
    window: String,
    algorithm_version: String,
}
impl ExperimentalMetadata {
    pub fn new(
        formula: impl Into<String>,
        baseline: impl Into<String>,
        window: impl Into<String>,
        algorithm_version: impl Into<String>,
    ) -> Result<Self, EventContractError> {
        let value = Self {
            formula: formula.into(),
            baseline: baseline.into(),
            window: window.into(),
            algorithm_version: algorithm_version.into(),
        };
        if [
            value.formula.is_empty(),
            value.baseline.is_empty(),
            value.window.is_empty(),
            value.algorithm_version.is_empty(),
        ]
        .iter()
        .any(|empty| *empty)
        {
            Err(EventContractError::MissingMetadata)
        } else {
            Ok(value)
        }
    }
    pub fn formula(&self) -> &str {
        &self.formula
    }
    pub fn baseline(&self) -> &str {
        &self.baseline
    }
    pub fn window(&self) -> &str {
        &self.window
    }
    pub fn algorithm_version(&self) -> &str {
        &self.algorithm_version
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventContractError {
    UnsupportedSchemaVersion(u16),
    SequenceNotIncreasing,
    TimestampDecreased,
    MissingMetadata,
}
