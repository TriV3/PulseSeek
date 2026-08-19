use std::f64::consts::TAU;

pub const GENERATOR_VERSION: &str = "pulseseek-calibration-v1";
pub const CHECKSUM_VERSION: &str = "crc32-ieee-canonical-v1";
pub const SAMPLE_RATES: [u32; 5] = [44_100, 48_000, 88_200, 96_000, 192_000];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixtureId {
    F001,
    F002,
    F003,
    F004,
    F005,
    F006,
    F007,
    F008,
    F009,
    F010,
    F011,
}

impl FixtureId {
    pub const ALL: [Self; 11] = [
        Self::F001,
        Self::F002,
        Self::F003,
        Self::F004,
        Self::F005,
        Self::F006,
        Self::F007,
        Self::F008,
        Self::F009,
        Self::F010,
        Self::F011,
    ];

    const fn code(self) -> &'static str {
        match self {
            Self::F001 => "F-001",
            Self::F002 => "F-002",
            Self::F003 => "F-003",
            Self::F004 => "F-004",
            Self::F005 => "F-005",
            Self::F006 => "F-006",
            Self::F007 => "F-007",
            Self::F008 => "F-008",
            Self::F009 => "F-009",
            Self::F010 => "F-010",
            Self::F011 => "F-011",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChannelVariant {
    Mono,
    Stereo,
    StereoIdentical,
    StereoInverted,
    LeftOnly,
    RightOnly,
}

impl ChannelVariant {
    pub const fn channel_count(self) -> u16 {
        match self {
            Self::Mono => 1,
            Self::Stereo
            | Self::StereoIdentical
            | Self::StereoInverted
            | Self::LeftOnly
            | Self::RightOnly => 2,
        }
    }

    const fn code(self) -> &'static str {
        match self {
            Self::Mono => "mono",
            Self::Stereo => "stereo",
            Self::StereoIdentical => "stereo-identical",
            Self::StereoInverted => "stereo-inverted",
            Self::LeftOnly => "left-only",
            Self::RightOnly => "right-only",
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Signal {
    Silence,
    Sine { frequency_hz: f64, level_dbfs: f64 },
    DualSine { first_hz: f64, second_hz: f64, level_dbfs: f64 },
    PinkNoise { seed: u64, level_dbfs: f64 },
    Impulse,
    InterSamplePeak,
}

#[derive(Clone, Debug)]
pub struct FixtureSpec {
    pub id: FixtureId,
    pub sample_rate: u32,
    pub channels: ChannelVariant,
    signal: Signal,
    frame_count: u32,
    expected_result: &'static str,
    tolerance: &'static str,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FixtureMetadata {
    pub id: FixtureId,
    pub generator_version: &'static str,
    pub checksum_version: &'static str,
    pub sample_rate: u32,
    pub channels: u16,
    pub channel_variant: ChannelVariant,
    pub frame_count: u32,
    pub duration_seconds: f64,
    pub level_dbfs: Option<f64>,
    pub signal_parameters: String,
    pub expected_result: &'static str,
    pub tolerance: &'static str,
    pub provenance: &'static str,
    pub license: &'static str,
    pub checksum: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeneratedFixture {
    pub metadata: FixtureMetadata,
    pub samples: Vec<f32>,
}

impl GeneratedFixture {
    pub fn checksum(&self) -> u32 {
        fixture_checksum(&self.metadata, &self.samples)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum FixtureError {
    InvalidFrameCount,
    NonFiniteSample,
    SizeOverflow,
}

pub fn catalogue() -> Vec<FixtureSpec> {
    let mut fixtures = Vec::new();
    for sample_rate in SAMPLE_RATES {
        let one_second = sample_rate;
        fixtures.extend([
            specification(
                FixtureId::F001,
                sample_rate,
                ChannelVariant::Mono,
                Signal::Silence,
                one_second,
                "digital silence",
                "exact zero",
            ),
            specification(
                FixtureId::F001,
                sample_rate,
                ChannelVariant::Stereo,
                Signal::Silence,
                one_second,
                "digital silence",
                "exact zero",
            ),
            specification(
                FixtureId::F002,
                sample_rate,
                ChannelVariant::Mono,
                Signal::Sine { frequency_hz: 1_000.0, level_dbfs: -18.0 },
                one_second,
                "1 kHz at -18 dBFS",
                "±0.1 dB",
            ),
            specification(
                FixtureId::F002,
                sample_rate,
                ChannelVariant::StereoIdentical,
                Signal::Sine { frequency_hz: 1_000.0, level_dbfs: -18.0 },
                one_second,
                "1 kHz at -18 dBFS",
                "±0.1 dB",
            ),
            specification(
                FixtureId::F003,
                sample_rate,
                ChannelVariant::Mono,
                Signal::Sine { frequency_hz: 30.0, level_dbfs: -18.0 },
                one_second,
                "30 Hz at -18 dBFS",
                "±0.1 dB",
            ),
            specification(
                FixtureId::F004,
                sample_rate,
                ChannelVariant::Mono,
                Signal::DualSine { first_hz: 50.0, second_hz: 10_000.0, level_dbfs: -24.0 },
                one_second,
                "50 Hz and 10 kHz",
                "within one FFT bin",
            ),
            specification(
                FixtureId::F005,
                sample_rate,
                ChannelVariant::Mono,
                Signal::PinkNoise { seed: 0x5055_4c53_4553_454b, level_dbfs: -18.0 },
                one_second,
                "deterministic pink noise normalized to -18 dBFS peak",
                "peak ±0.0001 dB",
            ),
            specification(
                FixtureId::F006,
                sample_rate,
                ChannelVariant::StereoIdentical,
                Signal::Sine { frequency_hz: 1_000.0, level_dbfs: -18.0 },
                one_second,
                "L=R correlation +1",
                "±0.001",
            ),
            specification(
                FixtureId::F007,
                sample_rate,
                ChannelVariant::StereoInverted,
                Signal::Sine { frequency_hz: 1_000.0, level_dbfs: -18.0 },
                one_second,
                "L=-R correlation -1",
                "±0.001",
            ),
            specification(
                FixtureId::F008,
                sample_rate,
                ChannelVariant::LeftOnly,
                Signal::Sine { frequency_hz: 1_000.0, level_dbfs: -18.0 },
                one_second,
                "left channel only",
                "inactive channel exact zero",
            ),
            specification(
                FixtureId::F009,
                sample_rate,
                ChannelVariant::RightOnly,
                Signal::Sine { frequency_hz: 1_000.0, level_dbfs: -18.0 },
                one_second,
                "right channel only",
                "inactive channel exact zero",
            ),
            specification(
                FixtureId::F010,
                sample_rate,
                ChannelVariant::Mono,
                Signal::Impulse,
                256,
                "unit impulse at frame 0",
                "sample-exact",
            ),
            specification(
                FixtureId::F010,
                sample_rate,
                ChannelVariant::StereoIdentical,
                Signal::Impulse,
                256,
                "unit impulse at frame 0",
                "sample-exact",
            ),
            specification(
                FixtureId::F011,
                sample_rate,
                ChannelVariant::Mono,
                Signal::InterSamplePeak,
                256,
                "continuous peak 0.99 (-0.0873 dBTP) over frames 32..224; sampled peak 0.99/√2",
                "future true-peak ±0.1 dBTP",
            ),
            specification(
                FixtureId::F011,
                sample_rate,
                ChannelVariant::StereoIdentical,
                Signal::InterSamplePeak,
                256,
                "continuous peak 0.99 (-0.0873 dBTP) over frames 32..224; sampled peak 0.99/√2",
                "future true-peak ±0.1 dBTP",
            ),
        ]);
    }
    fixtures
}

fn specification(
    id: FixtureId,
    sample_rate: u32,
    channels: ChannelVariant,
    signal: Signal,
    frame_count: u32,
    expected_result: &'static str,
    tolerance: &'static str,
) -> FixtureSpec {
    FixtureSpec { id, sample_rate, channels, signal, frame_count, expected_result, tolerance }
}

pub fn generate(specification: &FixtureSpec) -> Result<GeneratedFixture, FixtureError> {
    if specification.frame_count == 0 {
        return Err(FixtureError::InvalidFrameCount);
    }
    let sample_count = (specification.frame_count as usize)
        .checked_mul(specification.channels.channel_count() as usize)
        .ok_or(FixtureError::SizeOverflow)?;
    let mut samples = Vec::with_capacity(sample_count);
    let mut noise = PinkNoise::new(match specification.signal {
        Signal::PinkNoise { seed, .. } => seed,
        _ => 1,
    });
    let mut mono_samples = (0..specification.frame_count)
        .map(|frame| match specification.signal {
            Signal::Silence => 0.0,
            Signal::Sine { frequency_hz, level_dbfs } => {
                sine(frame, specification.sample_rate, frequency_hz, level_dbfs, 0.0)
            },
            Signal::DualSine { first_hz, second_hz, level_dbfs } => {
                sine(frame, specification.sample_rate, first_hz, level_dbfs, 0.0)
                    + sine(frame, specification.sample_rate, second_hz, level_dbfs, 0.0)
            },
            Signal::PinkNoise { .. } => noise.next(),
            Signal::Impulse => f64::from(frame == 0),
            Signal::InterSamplePeak => inter_sample_peak(frame),
        })
        .collect::<Vec<_>>();
    if let Signal::PinkNoise { level_dbfs, .. } = specification.signal {
        normalize_peak(&mut mono_samples, amplitude(level_dbfs));
    }
    for sample in mono_samples {
        push_channels(&mut samples, quantize(sample), specification.channels);
    }
    if samples.iter().any(|sample| !sample.is_finite()) {
        return Err(FixtureError::NonFiniteSample);
    }
    let (level_dbfs, signal_parameters) = describe_signal(specification.signal);
    let mut metadata = FixtureMetadata {
        id: specification.id,
        generator_version: GENERATOR_VERSION,
        checksum_version: CHECKSUM_VERSION,
        sample_rate: specification.sample_rate,
        channels: specification.channels.channel_count(),
        channel_variant: specification.channels,
        frame_count: specification.frame_count,
        duration_seconds: specification.frame_count as f64 / specification.sample_rate as f64,
        level_dbfs,
        signal_parameters,
        expected_result: specification.expected_result,
        tolerance: specification.tolerance,
        provenance: "PulseSeek mathematically generated fixture",
        license: "MPL-2.0",
        checksum: 0,
    };
    metadata.checksum = fixture_checksum(&metadata, &samples);
    Ok(GeneratedFixture { metadata, samples })
}

fn amplitude(level_dbfs: f64) -> f64 {
    10_f64.powf(level_dbfs / 20.0)
}

fn normalize_peak(samples: &mut [f64], target_peak: f64) {
    let peak = samples.iter().copied().map(f64::abs).fold(0.0, f64::max);
    for sample in samples {
        *sample = *sample / peak * target_peak;
    }
}

fn inter_sample_peak(frame: u32) -> f64 {
    const SAMPLED_PEAK: f64 = 0.700_035_713_374_682;
    match frame % 4 {
        0 | 1 => SAMPLED_PEAK,
        2 | 3 => -SAMPLED_PEAK,
        _ => unreachable!(),
    }
}

fn sine(frame: u32, sample_rate: u32, frequency_hz: f64, level_dbfs: f64, phase_turns: f64) -> f64 {
    (TAU * (frame as f64 * frequency_hz / sample_rate as f64 + phase_turns)).sin()
        * amplitude(level_dbfs)
}

fn quantize(sample: f64) -> f32 {
    const SCALE: f64 = 8_388_608.0;
    (sample.clamp(-1.0, 1.0 - 1.0 / SCALE) * SCALE).round() as f32 / SCALE as f32
}

fn push_channels(samples: &mut Vec<f32>, sample: f32, variant: ChannelVariant) {
    match variant {
        ChannelVariant::Mono => samples.push(sample),
        ChannelVariant::Stereo | ChannelVariant::StereoIdentical => {
            samples.extend([sample, sample])
        },
        ChannelVariant::StereoInverted => samples.extend([sample, -sample]),
        ChannelVariant::LeftOnly => samples.extend([sample, 0.0]),
        ChannelVariant::RightOnly => samples.extend([0.0, sample]),
    }
}

fn describe_signal(signal: Signal) -> (Option<f64>, String) {
    match signal {
        Signal::Silence => (None, "silence".into()),
        Signal::Sine { frequency_hz, level_dbfs } => {
            (Some(level_dbfs), format!("sine:{frequency_hz}Hz"))
        },
        Signal::DualSine { first_hz, second_hz, level_dbfs } => {
            (Some(level_dbfs), format!("dual-sine:{first_hz}Hz+{second_hz}Hz"))
        },
        Signal::PinkNoise { seed, level_dbfs } => {
            (Some(level_dbfs), format!("voss-mccartney:seed={seed:#x}:normalized=peak"))
        },
        Signal::Impulse => (Some(0.0), "impulse:frame=0".into()),
        Signal::InterSamplePeak => (Some(-0.087_296), "sine:fs/4:phase=1/8-turn".into()),
    }
}

struct PinkNoise {
    state: u64,
    rows: [i32; 16],
    counter: u32,
}

impl PinkNoise {
    fn new(seed: u64) -> Self {
        Self { state: seed, rows: [0; 16], counter: 0 }
    }

    fn next(&mut self) -> f64 {
        self.counter = self.counter.wrapping_add(1);
        let row = self.counter.trailing_zeros().min(15) as usize;
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.rows[row] = (self.state >> 32) as i32;
        let sum: i64 = self.rows.iter().map(|value| i64::from(*value)).sum();
        sum as f64 / (i32::MAX as f64 * self.rows.len() as f64)
    }
}

fn fixture_checksum(metadata: &FixtureMetadata, samples: &[f32]) -> u32 {
    let mut bytes = Vec::new();
    append_string(&mut bytes, metadata.id.code());
    append_string(&mut bytes, metadata.generator_version);
    append_string(&mut bytes, metadata.checksum_version);
    bytes.extend(metadata.sample_rate.to_le_bytes());
    bytes.extend(metadata.channels.to_le_bytes());
    append_string(&mut bytes, metadata.channel_variant.code());
    bytes.extend(metadata.frame_count.to_le_bytes());
    bytes.extend(metadata.duration_seconds.to_bits().to_le_bytes());
    bytes.extend(metadata.level_dbfs.map(f64::to_bits).unwrap_or(u64::MAX).to_le_bytes());
    append_string(&mut bytes, &metadata.signal_parameters);
    append_string(&mut bytes, metadata.expected_result);
    append_string(&mut bytes, metadata.tolerance);
    append_string(&mut bytes, metadata.provenance);
    append_string(&mut bytes, metadata.license);
    for sample in samples {
        bytes.extend(sample.to_bits().to_le_bytes());
    }
    crc32(&bytes)
}

fn append_string(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend((value.len() as u32).to_le_bytes());
    bytes.extend(value.as_bytes());
}

pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0_u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

pub fn wav_bytes(fixture: &GeneratedFixture) -> Vec<u8> {
    const BYTES_PER_SAMPLE: u16 = 3;
    let data_size = fixture.samples.len() as u32 * u32::from(BYTES_PER_SAMPLE);
    let mut bytes = Vec::with_capacity(44 + data_size as usize);
    bytes.extend(b"RIFF");
    bytes.extend((36 + data_size).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16_u32.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(fixture.metadata.channels.to_le_bytes());
    bytes.extend(fixture.metadata.sample_rate.to_le_bytes());
    let block_align = fixture.metadata.channels * BYTES_PER_SAMPLE;
    bytes.extend((fixture.metadata.sample_rate * u32::from(block_align)).to_le_bytes());
    bytes.extend(block_align.to_le_bytes());
    bytes.extend(24_u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(data_size.to_le_bytes());
    for sample in &fixture.samples {
        let encoded = (f64::from(*sample) * 8_388_608.0).round() as i32;
        bytes.extend(&encoded.to_le_bytes()[..3]);
    }
    bytes
}
