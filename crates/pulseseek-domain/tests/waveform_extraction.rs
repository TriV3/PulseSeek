use std::cell::Cell;

use pulseseek_domain::decoder::{DecodeError, Decoder, ProbeResult, StreamMetadata};
use pulseseek_domain::error::{DiagnosticCode, DiagnosticContext};
use pulseseek_domain::playback::position::{Duration, Position, SeekTarget};
use pulseseek_domain::waveform::extraction::{
    extract_overview, extract_window, ExtractionError, ExtractionOptions, WindowRequest,
};
use pulseseek_domain::waveform::peak::Peak;

/// A fake decoder that plays back pre-recorded PCM data with seek support.
struct FakeDecoder {
    data: Vec<f32>,
    position: usize,
    channels: u16,
    sample_rate: u32,
    duration: Duration,
    read_error: bool,
}

impl FakeDecoder {
    fn new(data: Vec<f32>, channels: u16, sample_rate: u32, duration_ms: u64) -> Self {
        Self {
            data,
            position: 0,
            channels,
            sample_rate,
            duration: Duration::from_millis(duration_ms),
            read_error: false,
        }
    }

    fn with_unknown_duration(data: Vec<f32>, channels: u16, sample_rate: u32) -> Self {
        Self {
            data,
            position: 0,
            channels,
            sample_rate,
            duration: Duration::Unknown,
            read_error: false,
        }
    }

    fn failing(mut self) -> Self {
        self.read_error = true;
        self
    }
}

impl Decoder for FakeDecoder {
    fn probe(&self) -> ProbeResult {
        ProbeResult::Supported
    }

    fn metadata(&mut self) -> Result<StreamMetadata, DecodeError> {
        Ok(StreamMetadata {
            sample_rate: self.sample_rate,
            channels: self.channels,
            duration: self.duration,
            bit_depth: None,
            codec: "test",
        })
    }

    fn read(&mut self, buf: &mut [f32]) -> Result<usize, DecodeError> {
        if self.read_error {
            return Err(DecodeError::new(
                DiagnosticContext::new(DiagnosticCode::BrowserRead),
                std::io::Error::other("fake corrupt stream"),
            ));
        }
        let remaining = self.data.len() - self.position;
        let to_copy = buf.len().min(remaining);
        buf[..to_copy].copy_from_slice(&self.data[self.position..self.position + to_copy]);
        self.position += to_copy;
        Ok(to_copy)
    }

    fn seek(&mut self, target: SeekTarget) -> Result<Position, DecodeError> {
        let frame = target.position().as_millis() * self.sample_rate as u64 / 1000;
        self.position = (frame * self.channels as u64) as usize;
        Ok(Position::from_millis(target.position().as_millis()))
    }
}

fn never_cancelled() -> &'static dyn Fn() -> bool {
    &|| false
}

fn peak(min: f32, max: f32) -> Peak {
    Peak::from_parts(min, max)
}

fn assert_peaks_eq(actual: &[Peak], expected: &[(f32, f32)]) {
    assert_eq!(actual.len(), expected.len(), "peak count mismatch");
    for (i, (got, want)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(*got, peak(want.0, want.1), "peak {i} mismatch");
    }
}

fn options(target_peak_count: u64, max_levels: u32) -> ExtractionOptions {
    ExtractionOptions::new(target_peak_count, max_levels).expect("valid options")
}

// ── Overview: known fixtures ───────────────────────────────────────

#[test]
fn overview_mono_known_fixture() {
    let mut decoder = FakeDecoder::new(vec![0.2, -0.5, 0.8, -0.1], 1, 1000, 4);
    let w = extract_overview(&mut decoder, &options(2, 4), never_cancelled()).expect("extract");
    assert_eq!(w.channels(), 1);
    assert_eq!(w.finest().samples_per_peak, 2);
    assert_peaks_eq(&w.finest().peaks, &[(-0.5, 0.2), (-0.1, 0.8)]);
    assert_peaks_eq(&w.coarsest().peaks, &[(-0.5, 0.8)]);
}

#[test]
fn overview_stereo_keeps_per_channel_envelope() {
    // Interleaved: L=0.3,-0.2 | R=-0.7,0.6. One bucket of 2 frames.
    let mut decoder = FakeDecoder::new(vec![0.3, -0.7, -0.2, 0.6], 2, 1000, 2);
    let w = extract_overview(&mut decoder, &options(1, 4), never_cancelled()).expect("extract");
    assert_eq!(w.channels(), 2);
    assert_eq!(w.finest().samples_per_peak, 2);
    assert_peaks_eq(&w.finest().peaks, &[(-0.2, 0.3), (-0.7, 0.6)]);
}

#[test]
fn overview_budget_derives_finest_samples_per_peak() {
    let mut decoder = FakeDecoder::new(vec![0.0; 1000], 1, 1000, 1000);
    let w = extract_overview(&mut decoder, &options(250, 4), never_cancelled()).expect("extract");
    assert_eq!(w.finest().samples_per_peak, 4);
    assert_eq!(w.finest().peaks.len(), 250);

    let mut decoder = FakeDecoder::new(vec![0.0; 1000], 1, 1000, 1000);
    let w = extract_overview(&mut decoder, &options(2000, 4), never_cancelled()).expect("extract");
    assert_eq!(w.finest().samples_per_peak, 1);
    assert_eq!(w.finest().peaks.len(), 1000);
}

#[test]
fn overview_pyramid_halves_until_one_bucket() {
    let mut decoder = FakeDecoder::new(vec![0.0; 8], 1, 1000, 8);
    let w = extract_overview(&mut decoder, &options(8, 8), never_cancelled()).expect("extract");
    let spp: Vec<u64> = w.levels().iter().map(|l| l.samples_per_peak).collect();
    let counts: Vec<usize> = w.levels().iter().map(|l| l.peaks.len()).collect();
    assert_eq!(spp, vec![8, 4, 2, 1]);
    assert_eq!(counts, vec![1, 2, 4, 8]);
}

#[test]
fn overview_respects_max_levels_cap() {
    let mut decoder = FakeDecoder::new(vec![0.0; 8], 1, 1000, 8);
    let w = extract_overview(&mut decoder, &options(8, 2), never_cancelled()).expect("extract");
    let spp: Vec<u64> = w.levels().iter().map(|l| l.samples_per_peak).collect();
    assert_eq!(spp, vec![2, 1]);
}

#[test]
fn overview_unknown_duration_reads_all_at_one_sample_per_peak() {
    let mut decoder = FakeDecoder::with_unknown_duration(vec![0.5, -0.5, 0.25], 1, 1000);
    let w = extract_overview(&mut decoder, &options(2, 4), never_cancelled()).expect("extract");
    assert_eq!(w.finest().samples_per_peak, 1);
    assert_peaks_eq(&w.finest().peaks, &[(0.5, 0.5), (-0.5, -0.5), (0.25, 0.25)]);
}

// ── Overview: cancellation ─────────────────────────────────────────

#[test]
fn overview_cancellation_stops_after_first_batch() {
    // Larger than one read batch so the cancellation check runs again.
    let mut decoder = FakeDecoder::new(vec![0.0; 20_000], 1, 1000, 20_000);
    let reads = Cell::new(0u32);
    let is_cancelled = || {
        reads.set(reads.get() + 1);
        reads.get() > 1
    };
    let err = extract_overview(&mut decoder, &options(1, 8), &is_cancelled)
        .expect_err("cancelled extraction");
    assert_eq!(err, ExtractionError::Cancelled);
    assert!(decoder.position < decoder.data.len(), "decoder must not be fully consumed");
}

// ── Overview: empty and corrupt ────────────────────────────────────

#[test]
fn overview_empty_source_rejected() {
    let mut decoder = FakeDecoder::new(vec![], 1, 1000, 0);
    let err = extract_overview(&mut decoder, &options(1, 8), never_cancelled()).expect_err("empty");
    assert_eq!(err, ExtractionError::EmptySource);
}

#[test]
fn overview_corrupt_source_propagates_decode_error() {
    let mut decoder = FakeDecoder::new(vec![0.0; 10], 1, 1000, 10).failing();
    let err =
        extract_overview(&mut decoder, &options(1, 8), never_cancelled()).expect_err("corrupt");
    assert!(matches!(err, ExtractionError::Decode(_)));
}

#[test]
fn overview_zero_channels_rejected() {
    let mut decoder = FakeDecoder::new(vec![0.0; 10], 0, 1000, 10);
    let err =
        extract_overview(&mut decoder, &options(1, 8), never_cancelled()).expect_err("no channels");
    assert_eq!(err, ExtractionError::UnsupportedSource);
}

// ── Window: sample-level precision ─────────────────────────────────

fn eight_samples() -> FakeDecoder {
    FakeDecoder::new(vec![0.1, -0.2, 0.3, -0.4, 0.5, -0.6, 0.7, -0.8], 1, 1000, 8)
}

#[test]
fn window_sample_level_returns_each_sample() {
    let mut decoder = eight_samples();
    let request = WindowRequest::new(0, 8, 1).expect("valid window");
    let peaks = extract_window(&mut decoder, &request, never_cancelled()).expect("extract");
    assert_peaks_eq(
        &peaks,
        &[
            (0.1, 0.1),
            (-0.2, -0.2),
            (0.3, 0.3),
            (-0.4, -0.4),
            (0.5, 0.5),
            (-0.6, -0.6),
            (0.7, 0.7),
            (-0.8, -0.8),
        ],
    );
}

#[test]
fn window_mid_file_returns_requested_slice() {
    let mut decoder = eight_samples();
    let request = WindowRequest::new(2, 3, 1).expect("valid window");
    let peaks = extract_window(&mut decoder, &request, never_cancelled()).expect("extract");
    assert_peaks_eq(&peaks, &[(0.3, 0.3), (-0.4, -0.4), (0.5, 0.5)]);
}

#[test]
fn window_bucketed() {
    let mut decoder = eight_samples();
    let request = WindowRequest::new(0, 8, 2).expect("valid window");
    let peaks = extract_window(&mut decoder, &request, never_cancelled()).expect("extract");
    assert_peaks_eq(&peaks, &[(-0.2, 0.1), (-0.4, 0.3), (-0.6, 0.5), (-0.8, 0.7)]);
}

#[test]
fn window_beyond_duration_returns_empty() {
    let mut decoder = eight_samples();
    let request = WindowRequest::new(1_000, 100, 1).expect("valid window");
    let peaks = extract_window(&mut decoder, &request, never_cancelled()).expect("extract");
    assert!(peaks.is_empty());
}

#[test]
fn window_partial_beyond_duration_returns_available() {
    let mut decoder = eight_samples();
    let request = WindowRequest::new(6, 100, 1).expect("valid window");
    let peaks = extract_window(&mut decoder, &request, never_cancelled()).expect("extract");
    assert_peaks_eq(&peaks, &[(0.7, 0.7), (-0.8, -0.8)]);
}

#[test]
fn window_zero_duration_returns_empty() {
    let mut decoder = eight_samples();
    let request = WindowRequest::new(0, 0, 1).expect("valid window");
    let peaks = extract_window(&mut decoder, &request, never_cancelled()).expect("extract");
    assert!(peaks.is_empty());
}

#[test]
fn window_cancellation_stops_after_first_batch() {
    // Larger than one read batch so the cancellation check runs again.
    let mut decoder = FakeDecoder::new(vec![0.0; 20_000], 1, 1000, 20_000);
    let reads = Cell::new(0u32);
    let is_cancelled = || {
        reads.set(reads.get() + 1);
        reads.get() > 1
    };
    let request = WindowRequest::new(0, 20_000, 1).expect("valid window");
    let err = extract_window(&mut decoder, &request, &is_cancelled).expect_err("cancelled");
    assert_eq!(err, ExtractionError::Cancelled);
}

#[test]
fn window_zero_sample_rate_rejected() {
    let mut decoder = FakeDecoder::new(vec![0.0; 10], 1, 0, 10);
    let request = WindowRequest::new(0, 5, 1).expect("valid window");
    let err = extract_window(&mut decoder, &request, never_cancelled()).expect_err("no rate");
    assert_eq!(err, ExtractionError::UnsupportedSource);
}

// ── Option validation ──────────────────────────────────────────────

#[test]
fn options_reject_zero_target_peaks() {
    let err = ExtractionOptions::new(0, 8).expect_err("zero target rejected");
    assert_eq!(err.to_string(), "target peak count must be positive");
}

#[test]
fn options_reject_zero_max_levels() {
    let err = ExtractionOptions::new(1, 0).expect_err("zero levels rejected");
    assert!(err.to_string().contains("max levels"));
}

#[test]
fn options_reject_excessive_max_levels() {
    let err = ExtractionOptions::new(1, 9).expect_err("excessive levels rejected");
    assert!(err.to_string().contains("max levels"));
}

#[test]
fn options_default_overview_is_valid() {
    let opts = ExtractionOptions::default_overview();
    assert!(opts.target_peak_count() > 0);
    assert!(opts.max_levels() >= 1);
}

#[test]
fn window_request_rejects_zero_samples_per_peak() {
    let err = WindowRequest::new(0, 100, 0).expect_err("zero spp rejected");
    assert_eq!(err.to_string(), "samples per peak must be positive");
}
