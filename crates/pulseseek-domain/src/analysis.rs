use std::{fmt, str::FromStr};

pub const ANALYSIS_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionRequest {
    source_id: SourceId,
    source_kind: SourceKind,
    measurement_point: MeasurementPoint,
    format: AudioFormat,
}

impl SessionRequest {
    pub fn new(
        source_id: SourceId,
        source_kind: SourceKind,
        measurement_point: MeasurementPoint,
        format: AudioFormat,
    ) -> Self {
        Self { source_id, source_kind, measurement_point, format }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionState {
    Idle,
    Running,
    Paused,
    Incomplete,
    Stopped,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SessionEvent {
    Start(SessionRequest),
    AudioBlock(AnalysisBlock),
    Pause,
    Resume,
    LoopWrap,
    Seek { first_sample: u64 },
    FormatChange(AudioFormat),
    SourceChange { source_id: SourceId, measurement_point: MeasurementPoint },
    Gap { first_sample: u64, frame_count: u64 },
    Stop,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceSession {
    id: SessionId,
    request: SessionRequest,
    state: SessionState,
    next_sample: u64,
    next_sequence: u64,
    loop_wrapped: bool,
    last_gap: Option<(u64, u64)>,
}

impl SourceSession {
    fn start_new_session(
        &mut self,
        format: AudioFormat,
        source_id: SourceId,
        measurement_point: MeasurementPoint,
        first_sample: u64,
    ) {
        self.id = SessionId::new(format!("{}-next", self.id.0));
        self.request.format = format;
        self.request.source_id = source_id;
        self.request.measurement_point = measurement_point;
        self.state = SessionState::Running;
        self.next_sample = first_sample;
        self.next_sequence = 0;
        self.loop_wrapped = false;
        self.last_gap = None;
    }

    pub fn id(&self) -> &SessionId {
        &self.id
    }
    pub fn state(&self) -> SessionState {
        self.state
    }
    pub fn next_sample(&self) -> u64 {
        self.next_sample
    }
    pub fn next_sequence(&self) -> u64 {
        self.next_sequence
    }
    pub fn loop_wrapped(&self) -> bool {
        self.loop_wrapped
    }
    pub fn last_gap(&self) -> Option<(u64, u64)> {
        self.last_gap
    }
    pub fn apply(&mut self, event: SessionEvent) -> Result<SessionState, AnalysisError> {
        match event {
            SessionEvent::Start(_) => return Err(AnalysisError::DuplicateStart),
            SessionEvent::Pause if self.state == SessionState::Running => {
                self.state = SessionState::Paused
            },
            SessionEvent::Resume if self.state == SessionState::Paused => {
                self.state = SessionState::Running
            },
            SessionEvent::LoopWrap
                if matches!(self.state, SessionState::Running | SessionState::Incomplete) =>
            {
                self.loop_wrapped = true;
            },
            SessionEvent::Stop if self.state != SessionState::Stopped => {
                self.state = SessionState::Stopped
            },
            SessionEvent::Gap { first_sample, frame_count }
                if matches!(self.state, SessionState::Running | SessionState::Incomplete) =>
            {
                self.next_sample = first_sample
                    .checked_add(frame_count)
                    .ok_or(AnalysisError::InvalidFrameCount)?;
                self.next_sequence =
                    self.next_sequence.checked_add(1).ok_or(AnalysisError::InvalidFrameCount)?;
                self.last_gap = Some((first_sample, frame_count));
                self.state = SessionState::Incomplete;
            },
            SessionEvent::AudioBlock(block)
                if matches!(self.state, SessionState::Running | SessionState::Incomplete) =>
            {
                if block.source_id() != &self.request.source_id
                    || block.session_id() != &self.id
                    || block.source_kind() != self.request.source_kind
                    || block.measurement_point() != self.request.measurement_point
                    || block.format() != self.request.format
                {
                    return Err(AnalysisError::InvalidEvent);
                }
                if (self.state == SessionState::Running && block.first_sample() != self.next_sample)
                    || (self.state == SessionState::Incomplete && !block.discontinuity())
                    || block.sequence() != self.next_sequence
                {
                    return Err(AnalysisError::CounterDiscontinuity);
                }
                self.next_sample = self
                    .next_sample
                    .checked_add(block.frame_count())
                    .ok_or(AnalysisError::InvalidFrameCount)?;
                self.next_sequence =
                    self.next_sequence.checked_add(1).ok_or(AnalysisError::InvalidFrameCount)?;
            },
            SessionEvent::Seek { first_sample } if self.state != SessionState::Stopped => {
                self.start_new_session(
                    self.request.format,
                    self.request.source_id.clone(),
                    self.request.measurement_point,
                    first_sample,
                );
            },
            SessionEvent::FormatChange(format) if self.state != SessionState::Stopped => {
                self.start_new_session(
                    format,
                    self.request.source_id.clone(),
                    self.request.measurement_point,
                    0,
                );
            },
            SessionEvent::SourceChange { source_id, measurement_point }
                if self.state != SessionState::Stopped =>
            {
                self.start_new_session(self.request.format, source_id, measurement_point, 0);
            },
            _ => return Err(AnalysisError::InvalidEvent),
        }
        Ok(self.state)
    }
}

pub trait AudioAnalysisSource {
    fn start(&mut self, request: SessionRequest) -> Result<SourceSession, AnalysisError>;
    fn start_event(
        &mut self,
        request: SessionRequest,
    ) -> Result<(SourceSession, SessionEvent), AnalysisError> {
        let session = self.start(request.clone())?;
        Ok((session, SessionEvent::Start(request)))
    }
}

#[derive(Default)]
pub struct InMemoryAnalysisSource {
    next_id: u64,
}

impl InMemoryAnalysisSource {
    pub fn new() -> Self {
        Self::default()
    }
}

impl AudioAnalysisSource for InMemoryAnalysisSource {
    fn start(&mut self, request: SessionRequest) -> Result<SourceSession, AnalysisError> {
        self.next_id = self.next_id.checked_add(1).ok_or(AnalysisError::InvalidFrameCount)?;
        Ok(SourceSession {
            id: SessionId::new(format!("analysis-{}", self.next_id)),
            request,
            state: SessionState::Running,
            next_sample: 0,
            next_sequence: 0,
            loop_wrapped: false,
            last_gap: None,
        })
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
    InvalidEvent,
    CounterDiscontinuity,
    SessionResetRequired,
    DuplicateStart,
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
