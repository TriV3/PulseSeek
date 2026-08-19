use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use pulseseek_domain::analysis::{
    AnalysisBlock, AnalysisError, AudioFormat, ChannelLayout, MeasurementPoint, SessionId,
    SourceId, SourceKind,
};
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};

pub const MAX_ANALYSIS_CAPTURE_SAMPLES: usize = 8_192;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnalysisCaptureConfig {
    source_id: SourceId,
    session_id: SessionId,
    source_kind: SourceKind,
    measurement_point: MeasurementPoint,
    format: AudioFormat,
}

impl AnalysisCaptureConfig {
    pub fn new(
        source_id: SourceId,
        session_id: SessionId,
        source_kind: SourceKind,
        measurement_point: MeasurementPoint,
        format: AudioFormat,
    ) -> Self {
        Self { source_id, session_id, source_kind, measurement_point, format }
    }

    pub fn try_new(
        source_id: SourceId,
        session_id: SessionId,
        source_kind: SourceKind,
        measurement_point: MeasurementPoint,
        sample_rate: u32,
        channels: u16,
    ) -> Result<Self, AnalysisError> {
        let layout = match channels {
            1 => ChannelLayout::Mono,
            2 => ChannelLayout::Stereo,
            channels => ChannelLayout::MultiChannel(channels),
        };
        Ok(Self::new(
            source_id,
            session_id,
            source_kind,
            measurement_point,
            AudioFormat::new(sample_rate, layout)?,
        ))
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CaptureSaturation {
    pub dropped_blocks: u64,
    pub dropped_frames: u64,
}

struct SaturationCounters {
    dropped_blocks: AtomicU64,
    dropped_frames: AtomicU64,
}

impl SaturationCounters {
    fn snapshot(&self) -> CaptureSaturation {
        CaptureSaturation {
            dropped_blocks: self.dropped_blocks.load(Ordering::Relaxed),
            dropped_frames: self.dropped_frames.load(Ordering::Relaxed),
        }
    }
}

struct CapturedBlock {
    config: Arc<AnalysisCaptureConfig>,
    source_generation: u64,
    session_generation: u64,
    first_sample: u64,
    frame_count: u64,
    sequence: u64,
    discontinuity: bool,
    sample_count: usize,
    samples: [f32; MAX_ANALYSIS_CAPTURE_SAMPLES],
}

pub fn analysis_capture_channel(
    capacity: usize,
    config: AnalysisCaptureConfig,
) -> (AnalysisCaptureProducer, AnalysisCaptureConsumer) {
    assert!(capacity > 0, "analysis capture capacity must be positive");
    let (producer, consumer) = HeapRb::new(capacity).split();
    let shutdown = Arc::new(AtomicBool::new(false));
    let saturation = Arc::new(SaturationCounters {
        dropped_blocks: AtomicU64::new(0),
        dropped_frames: AtomicU64::new(0),
    });
    let config = Arc::new(config);
    (
        AnalysisCaptureProducer {
            producer,
            config: Arc::clone(&config),
            shutdown: Arc::clone(&shutdown),
            saturation: Arc::clone(&saturation),
            next_sequence: 0,
            source_generation: 0,
            session_generation: 0,
            pending_discontinuity: false,
        },
        AnalysisCaptureConsumer { consumer, shutdown, saturation },
    )
}

pub struct AnalysisCaptureProducer {
    producer: HeapProd<CapturedBlock>,
    config: Arc<AnalysisCaptureConfig>,
    shutdown: Arc<AtomicBool>,
    saturation: Arc<SaturationCounters>,
    next_sequence: u64,
    source_generation: u64,
    session_generation: u64,
    pending_discontinuity: bool,
}

impl AnalysisCaptureProducer {
    pub fn try_capture(
        &mut self,
        first_sample: u64,
        samples: &[f32],
        discontinuity: bool,
    ) -> CaptureResult {
        if self.is_shutdown() {
            return CaptureResult::Shutdown;
        }
        if !self.producer.read_is_held() {
            return CaptureResult::ConsumerGone;
        }
        let channels = usize::from(self.config.format.channels());
        if samples.is_empty()
            || samples.len() > MAX_ANALYSIS_CAPTURE_SAMPLES
            || !samples.len().is_multiple_of(channels)
            || samples.iter().any(|sample| !sample.is_finite())
        {
            return CaptureResult::InvalidBlock;
        }
        let frame_count = samples.len() / channels;
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        if self.producer.is_full() {
            self.saturation.dropped_blocks.fetch_add(1, Ordering::Relaxed);
            self.saturation
                .dropped_frames
                .fetch_add(u64::try_from(frame_count).unwrap_or(u64::MAX), Ordering::Relaxed);
            self.pending_discontinuity = true;
            return CaptureResult::DroppedFull;
        }
        let mut storage = [0.0; MAX_ANALYSIS_CAPTURE_SAMPLES];
        storage[..samples.len()].copy_from_slice(samples);
        let block = CapturedBlock {
            config: Arc::clone(&self.config),
            source_generation: self.source_generation,
            session_generation: self.session_generation,
            first_sample,
            frame_count: u64::try_from(frame_count).unwrap_or(u64::MAX),
            sequence,
            discontinuity: discontinuity || self.pending_discontinuity,
            sample_count: samples.len(),
            samples: storage,
        };
        match self.producer.try_push(block) {
            Ok(()) => {
                self.pending_discontinuity = false;
                CaptureResult::Captured
            },
            Err(_) => {
                self.saturation.dropped_blocks.fetch_add(1, Ordering::Relaxed);
                self.saturation
                    .dropped_frames
                    .fetch_add(u64::try_from(frame_count).unwrap_or(u64::MAX), Ordering::Relaxed);
                self.pending_discontinuity = true;
                CaptureResult::DroppedFull
            },
        }
    }

    pub fn try_capture_bounded(&mut self, first_sample: u64, samples: &[f32], discontinuity: bool) {
        let channels = usize::from(self.config.format.channels());
        let chunk_size = MAX_ANALYSIS_CAPTURE_SAMPLES - MAX_ANALYSIS_CAPTURE_SAMPLES % channels;
        for (index, chunk) in samples.chunks(chunk_size).enumerate() {
            let frame_offset = index.saturating_mul(chunk_size) / channels;
            let _ = self.try_capture(
                first_sample.saturating_add(u64::try_from(frame_offset).unwrap_or(u64::MAX)),
                chunk,
                discontinuity && index == 0,
            );
        }
    }

    pub fn channels(&self) -> u16 {
        self.config.format.channels()
    }

    pub fn sample_rate(&self) -> u32 {
        self.config.format.sample_rate()
    }

    pub fn start_new_session(&mut self) {
        self.session_generation = self.session_generation.wrapping_add(1);
        self.next_sequence = 0;
        self.pending_discontinuity = false;
    }

    pub fn start_new_session_at(&mut self, first_sample: u64) {
        self.start_new_session();
        self.pending_discontinuity = first_sample > 0;
    }

    pub fn rotate_source(&mut self) {
        self.source_generation = self.source_generation.wrapping_add(1);
        self.session_generation = self.session_generation.wrapping_add(1);
        self.next_sequence = 0;
        self.pending_discontinuity = false;
    }

    pub fn start_new_source(&mut self, sample_rate: u32) -> Result<(), AnalysisError> {
        let channels = self.config.format.channels();
        let layout = if channels == 1 { ChannelLayout::Mono } else { ChannelLayout::Stereo };
        self.source_generation = self.source_generation.wrapping_add(1);
        self.session_generation = self.session_generation.wrapping_add(1);
        let config = Arc::make_mut(&mut self.config);
        config.format = AudioFormat::new(sample_rate, layout)?;
        self.next_sequence = 0;
        self.pending_discontinuity = false;
        Ok(())
    }

    pub fn saturation(&self) -> CaptureSaturation {
        self.saturation.snapshot()
    }

    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Release);
    }

    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureResult {
    Captured,
    DroppedFull,
    InvalidBlock,
    ConsumerGone,
    Shutdown,
}

pub struct AnalysisCaptureConsumer {
    consumer: HeapCons<CapturedBlock>,
    shutdown: Arc<AtomicBool>,
    saturation: Arc<SaturationCounters>,
}

impl AnalysisCaptureConsumer {
    pub fn try_receive(&mut self) -> Option<AnalysisBlock> {
        let captured = self.consumer.try_pop()?;
        AnalysisBlock::new(
            captured.config.source_id.successor_by(captured.source_generation),
            captured.config.session_id.successor_by(captured.session_generation),
            captured.config.source_kind,
            captured.config.measurement_point,
            captured.config.format,
            captured.first_sample,
            captured.frame_count,
            captured.sequence,
            captured.discontinuity,
            captured.samples[..captured.sample_count].to_vec(),
        )
        .ok()
    }

    pub fn saturation(&self) -> CaptureSaturation {
        self.saturation.snapshot()
    }

    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Release);
    }

    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }

    pub fn is_closed(&self) -> bool {
        self.is_shutdown() || (!self.consumer.write_is_held() && self.consumer.is_empty())
    }
}
