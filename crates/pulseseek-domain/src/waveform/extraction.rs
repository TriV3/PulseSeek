use std::fmt;

use crate::decoder::{DecodeError, Decoder};
use crate::playback::position::{Duration, Position};
use crate::waveform::levels::{Level, LevelIndex, MultiresolutionWaveform, MAX_LEVELS};
use crate::waveform::peak::Peak;

/// Default target bucket count for the finest display overview (per channel).
///
/// The renderer requests roughly two buckets per CSS pixel. Keeping a bounded
/// pyramid at 8,192 buckets covers a 4,096-pixel-wide canvas without decoding,
/// reducing, and serializing detail the current renderer cannot display.
/// Coarser levels still preserve the complete file overview.
pub const DEFAULT_TARGET_PEAK_COUNT: u64 = 8_192;

/// Number of buckets used for the first, sampled preview.
pub const FAST_PREVIEW_PEAK_COUNT: u64 = 64;

/// Maximum bucket count for a sampled cache-miss response.
pub const MAX_SAMPLED_PREVIEW_PEAK_COUNT: u64 = 2_048;

/// Maximum coarse seeks needed for any sampled preview.
const SAMPLED_PREVIEW_MAX_SEEKS: u64 = 4;

/// Contiguous frames decoded for each locally-derived preview bucket.
const PREVIEW_FRAMES_PER_BUCKET: u64 = 256;

/// Maximum allocation for a sampled preview window across every channel.
const MAX_PREVIEW_WINDOW_SAMPLES: usize = 65_536;

/// Interleaved samples read from the decoder in one batch.
const READ_BATCH_SAMPLES: usize = 8192;

/// Validated options controlling overview peak extraction.
///
/// The finest level's samples-per-peak is derived from the source length and
/// [`ExtractionOptions::target_peak_count`] so that the resulting pyramid is
/// bounded no matter how long the audio is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExtractionOptions {
    target_peak_count: u64,
    max_levels: u32,
}

impl ExtractionOptions {
    /// Creates options with a validated peak budget and level cap.
    ///
    /// `target_peak_count` must be positive and `max_levels` must lie within
    /// `1..=MAX_LEVELS`.
    pub fn new(target_peak_count: u64, max_levels: u32) -> Result<Self, ExtractionOptionsError> {
        if target_peak_count == 0 {
            return Err(ExtractionOptionsError::ZeroTargetPeaks);
        }
        if max_levels == 0 || max_levels > MAX_LEVELS {
            return Err(ExtractionOptionsError::InvalidMaxLevels(max_levels));
        }
        Ok(Self { target_peak_count, max_levels })
    }

    /// Sensible defaults for a full-file overview.
    pub const fn default_overview() -> Self {
        Self { target_peak_count: DEFAULT_TARGET_PEAK_COUNT, max_levels: MAX_LEVELS }
    }

    /// Desired number of buckets in the finest level (per channel).
    pub const fn target_peak_count(&self) -> u64 {
        self.target_peak_count
    }

    /// Maximum number of resolution levels in the pyramid.
    pub const fn max_levels(&self) -> u32 {
        self.max_levels
    }
}

/// Validation error for [`ExtractionOptions`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExtractionOptionsError {
    ZeroTargetPeaks,
    InvalidMaxLevels(u32),
}

impl fmt::Display for ExtractionOptionsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroTargetPeaks => write!(f, "target peak count must be positive"),
            Self::InvalidMaxLevels(value) => {
                write!(f, "max levels must be between 1 and {MAX_LEVELS}, got {value}")
            },
        }
    }
}

impl std::error::Error for ExtractionOptionsError {}

/// Validated time window used for focused, deep-zoom extraction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowRequest {
    start_ms: u64,
    duration_ms: u64,
    samples_per_peak: u64,
}

impl WindowRequest {
    /// Creates a window request.
    ///
    /// `samples_per_peak` must be positive. A zero `duration_ms` is valid and
    /// produces an empty result.
    pub fn new(
        start_ms: u64,
        duration_ms: u64,
        samples_per_peak: u64,
    ) -> Result<Self, WindowRequestError> {
        if samples_per_peak == 0 {
            return Err(WindowRequestError);
        }
        Ok(Self { start_ms, duration_ms, samples_per_peak })
    }

    /// Start of the window in milliseconds.
    pub const fn start_ms(&self) -> u64 {
        self.start_ms
    }

    /// Length of the window in milliseconds.
    pub const fn duration_ms(&self) -> u64 {
        self.duration_ms
    }

    /// Frames covered by one peak bucket inside the window.
    pub const fn samples_per_peak(&self) -> u64 {
        self.samples_per_peak
    }
}

/// Validation error for [`WindowRequest`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowRequestError;

impl fmt::Display for WindowRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "samples per peak must be positive")
    }
}

impl std::error::Error for WindowRequestError {}

/// Error produced by waveform extraction.
#[derive(Debug)]
pub enum ExtractionError {
    /// Extraction was cancelled by the caller.
    Cancelled,
    /// The source contains no audio frames.
    EmptySource,
    /// The source cannot be analyzed (no channels or no sample rate).
    UnsupportedSource,
    /// Decoding failed while reading frames.
    Decode(DecodeError),
}

impl PartialEq for ExtractionError {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Cancelled, Self::Cancelled)
                | (Self::EmptySource, Self::EmptySource)
                | (Self::UnsupportedSource, Self::UnsupportedSource)
        )
    }
}

impl fmt::Display for ExtractionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => write!(f, "waveform extraction cancelled"),
            Self::EmptySource => write!(f, "audio source contains no frames"),
            Self::UnsupportedSource => write!(f, "audio source is not analyzable"),
            Self::Decode(error) => write!(f, "waveform extraction failed: {error}"),
        }
    }
}

impl std::error::Error for ExtractionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Decode(error) => Some(error),
            _ => None,
        }
    }
}

/// Extracts a bounded multiresolution overview pyramid from a decoder.
///
/// The finest level keeps one envelope per channel (interleaved by bucket).
/// Coarser levels are derived by merging adjacent buckets in halves. The
/// `is_cancelled` check runs before every read batch; cancellation aborts the
/// extraction with [`ExtractionError::Cancelled`].
pub fn extract_overview(
    decoder: &mut dyn Decoder,
    options: &ExtractionOptions,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<MultiresolutionWaveform, ExtractionError> {
    let meta = decoder.metadata().map_err(ExtractionError::Decode)?;
    let channels = meta.channels;
    if channels == 0 {
        return Err(ExtractionError::UnsupportedSource);
    }

    let total_frames = match meta.duration {
        Duration::Known(position) if meta.sample_rate > 0 => {
            ((position.as_millis() as u128 * meta.sample_rate as u128) / 1000).min(u64::MAX as u128)
                as u64
        },
        _ => 0,
    };
    let finest_spp = if total_frames > 0 {
        let spp = (total_frames as u128).div_ceil(options.target_peak_count() as u128);
        spp.max(1).min(u64::MAX as u128) as u64
    } else {
        1
    };

    let channels_usize = channels as usize;
    let fine_peaks = read_all_peaks(decoder, channels, finest_spp, is_cancelled)?;
    if fine_peaks.is_empty() {
        return Err(ExtractionError::EmptySource);
    }

    let mut levels_rev = vec![(finest_spp, fine_peaks)];
    while levels_rev.len() < options.max_levels() as usize {
        let (next_spp, coarse) = {
            let (spp, peaks) = levels_rev.last().expect("non-empty level list");
            if peaks.len() / channels_usize <= 1 {
                break;
            }
            let Some(next_spp) = spp.checked_mul(2) else { break };
            (next_spp, reduce_half(peaks, channels_usize))
        };
        levels_rev.push((next_spp, coarse));
    }
    levels_rev.reverse();

    let levels = levels_rev
        .into_iter()
        .enumerate()
        .map(|(index, (samples_per_peak, peaks))| Level {
            index: LevelIndex::new(index as u32).expect("level count within MAX_LEVELS"),
            samples_per_peak,
            peaks,
        })
        .collect();
    Ok(MultiresolutionWaveform::from_levels(channels, levels)
        .expect("extraction builds a valid waveform"))
}

/// Extracts a fast, approximate overview from a few evenly spaced windows.
///
/// Several adjacent buckets are derived from each contiguous read so the
/// number of expensive decoder seeks stays bounded independently from the
/// requested draw resolution. This intentionally does not read the complete
/// source. It is suitable only for first paint; callers should replace it with
/// [`extract_overview`] when the exact pyramid is available.
pub fn extract_sampled_overview(
    decoder: &mut dyn Decoder,
    target_buckets: u64,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<MultiresolutionWaveform, ExtractionError> {
    let meta = decoder.metadata().map_err(ExtractionError::Decode)?;
    let channels = meta.channels;
    if channels == 0 || meta.sample_rate == 0 {
        return Err(ExtractionError::UnsupportedSource);
    }
    let duration_ms = match meta.duration {
        Duration::Known(position) if position.as_millis() > 0 => position.as_millis(),
        _ => return Err(ExtractionError::UnsupportedSource),
    };
    let buckets = target_buckets.clamp(1, MAX_SAMPLED_PREVIEW_PEAK_COUNT);
    let channel_count = channels as usize;
    let total_frames = ((duration_ms as u128 * meta.sample_rate as u128) / 1000)
        .max(1)
        .min(u64::MAX as u128) as u64;
    let samples_per_peak =
        (total_frames as u128).div_ceil(buckets as u128).min(u64::MAX as u128) as u64;
    let max_window_frames = (MAX_PREVIEW_WINDOW_SAMPLES / channel_count).max(1) as u64;
    let seek_count = buckets.min(SAMPLED_PREVIEW_MAX_SEEKS);
    let mut peaks = Vec::with_capacity(buckets as usize * channel_count);
    let mut buffer = vec![0.0f32; max_window_frames as usize * channel_count];
    let mut min = vec![f32::INFINITY; channel_count];
    let mut max = vec![f32::NEG_INFINITY; channel_count];

    for window in 0..seek_count {
        if is_cancelled() {
            return Err(ExtractionError::Cancelled);
        }
        let first_bucket = window * buckets / seek_count;
        let next_bucket = (window + 1) * buckets / seek_count;
        let local_buckets = next_bucket - first_bucket;
        let region_start_frame =
            (window as u128 * total_frames as u128 / seek_count as u128) as u64;
        let region_end_frame =
            ((window + 1) as u128 * total_frames as u128 / seek_count as u128) as u64;
        let region_frames = region_end_frame.saturating_sub(region_start_frame).max(1);
        let desired_window_frames = local_buckets.saturating_mul(PREVIEW_FRAMES_PER_BUCKET);
        let window_frames = desired_window_frames.min(region_frames).min(max_window_frames).max(1);
        let start_frame = region_start_frame + (region_frames - window_frames) / 2;
        let start_ms = (start_frame as u128 * 1000 / meta.sample_rate as u128) as u64;
        let target = meta
            .duration
            .seek_to(Position::from_millis(start_ms))
            .expect("sample position stays within source duration");
        decoder.seek_coarse(target).map_err(ExtractionError::Decode)?;
        let window_samples = window_frames as usize * channel_count;
        let samples =
            decoder.read(&mut buffer[..window_samples]).map_err(ExtractionError::Decode)?;
        let decoded_frames = samples / channel_count;

        for local_bucket in 0..local_buckets as usize {
            let start = local_bucket * decoded_frames / local_buckets as usize;
            let end = (local_bucket + 1) * decoded_frames / local_buckets as usize;
            if start == end {
                peaks.extend((0..channel_count).map(|_| Peak::from_parts(0.0, 0.0)));
                continue;
            }
            min.fill(f32::INFINITY);
            max.fill(f32::NEG_INFINITY);
            for frame in start..end {
                for channel in 0..channel_count {
                    let sample = buffer[frame * channel_count + channel];
                    min[channel] = min[channel].min(sample);
                    max[channel] = max[channel].max(sample);
                }
            }
            for channel in 0..channel_count {
                peaks.push(Peak::from_parts(min[channel], max[channel]));
            }
        }
    }

    let level =
        Level { index: LevelIndex::new(0).expect("level zero is valid"), samples_per_peak, peaks };
    Ok(MultiresolutionWaveform::from_levels(channels, vec![level])
        .expect("sampled preview is structurally valid"))
}

/// Extracts peaks for a focused time window, supporting sample-level zoom.
///
/// The decoder is seeked to the window start and only the requested span is
/// read, so work stays proportional to the window size. `samples_per_peak` of
/// one returns every sample as its own peak. A window that starts beyond the
/// source duration produces an empty result.
pub fn extract_window(
    decoder: &mut dyn Decoder,
    request: &WindowRequest,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Peak>, ExtractionError> {
    let meta = decoder.metadata().map_err(ExtractionError::Decode)?;
    let channels = meta.channels;
    if channels == 0 || meta.sample_rate == 0 {
        return Err(ExtractionError::UnsupportedSource);
    }
    if request.duration_ms() == 0 {
        return Ok(Vec::new());
    }

    let Ok(target) = meta.duration.seek_to(Position::from_millis(request.start_ms())) else {
        return Ok(Vec::new());
    };
    decoder.seek(target).map_err(ExtractionError::Decode)?;

    let channel_count = channels as usize;
    let window_frames = ((request.duration_ms() as u128 * meta.sample_rate as u128) / 1000)
        .min(u64::MAX as u128) as u64;
    let samples_to_read = (window_frames as u128 * channels as u128).min(u64::MAX as u128) as u64;
    let mut peaks: Vec<Peak> = Vec::new();
    let mut accumulator = BucketAccumulator::new(channel_count, request.samples_per_peak());
    let mut buffer = vec![0.0f32; READ_BATCH_SAMPLES];
    let mut remaining = samples_to_read;

    while remaining > 0 {
        if is_cancelled() {
            return Err(ExtractionError::Cancelled);
        }
        let want = remaining.min(READ_BATCH_SAMPLES as u64) as usize;
        let samples = decoder.read(&mut buffer[..want]).map_err(ExtractionError::Decode)?;
        if samples == 0 {
            break;
        }
        for frame in 0..samples / channel_count {
            let base = frame * channel_count;
            accumulator.add_frame(&buffer[base..base + channel_count], &mut peaks);
        }
        remaining -= samples as u64;
    }
    accumulator.flush(&mut peaks);
    Ok(peaks)
}

/// Reads the full source and buckets peaks per channel.
fn read_all_peaks(
    decoder: &mut dyn Decoder,
    channels: u16,
    samples_per_peak: u64,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Peak>, ExtractionError> {
    let channel_count = channels as usize;
    let mut peaks: Vec<Peak> = Vec::new();
    let mut accumulator = BucketAccumulator::new(channel_count, samples_per_peak);
    let mut buffer = vec![0.0f32; READ_BATCH_SAMPLES];

    loop {
        if is_cancelled() {
            return Err(ExtractionError::Cancelled);
        }
        let samples = decoder.read(&mut buffer).map_err(ExtractionError::Decode)?;
        if samples == 0 {
            break;
        }
        for frame in 0..samples / channel_count {
            let base = frame * channel_count;
            accumulator.add_frame(&buffer[base..base + channel_count], &mut peaks);
        }
    }
    accumulator.flush(&mut peaks);
    Ok(peaks)
}

/// Accumulates one envelope per channel over `samples_per_peak` frames.
struct BucketAccumulator {
    min: Vec<f32>,
    max: Vec<f32>,
    frames_in_bucket: u64,
    samples_per_peak: u64,
    have_bucket: bool,
}

impl BucketAccumulator {
    fn new(channels: usize, samples_per_peak: u64) -> Self {
        Self {
            min: vec![0.0f32; channels],
            max: vec![0.0f32; channels],
            frames_in_bucket: 0,
            samples_per_peak,
            have_bucket: false,
        }
    }

    fn add_frame(&mut self, frame: &[f32], out: &mut Vec<Peak>) {
        for (channel, &sample) in frame.iter().enumerate() {
            if self.have_bucket {
                if sample < self.min[channel] {
                    self.min[channel] = sample;
                }
                if sample > self.max[channel] {
                    self.max[channel] = sample;
                }
            } else {
                self.min[channel] = sample;
                self.max[channel] = sample;
            }
        }
        self.frames_in_bucket += 1;
        if self.frames_in_bucket == self.samples_per_peak {
            self.flush(out);
        } else {
            self.have_bucket = true;
        }
    }

    fn flush(&mut self, out: &mut Vec<Peak>) {
        if !self.have_bucket && self.frames_in_bucket == 0 {
            return;
        }
        for channel in 0..self.min.len() {
            out.push(Peak::from_parts(self.min[channel], self.max[channel]));
        }
        self.frames_in_bucket = 0;
        self.have_bucket = false;
    }
}

/// Merges adjacent buckets in halves while preserving per-channel interleaving.
fn reduce_half(peaks: &[Peak], channels: usize) -> Vec<Peak> {
    let buckets = peaks.len() / channels;
    let out_buckets = buckets.div_ceil(2);
    let mut out = Vec::with_capacity(out_buckets * channels);
    for bucket in 0..out_buckets {
        for channel in 0..channels {
            let first = peaks[bucket * 2 * channels + channel];
            let has_pair = bucket * 2 + 1 < buckets;
            let combined = if has_pair {
                let second = peaks[(bucket * 2 + 1) * channels + channel];
                Peak::from_parts(first.min().min(second.min()), first.max().max(second.max()))
            } else {
                first
            };
            out.push(combined);
        }
    }
    out
}
