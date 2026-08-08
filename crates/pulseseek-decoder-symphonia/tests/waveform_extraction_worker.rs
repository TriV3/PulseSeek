use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::mpsc;

use pulseseek_decoder_symphonia::registry::DecoderRegistry;
use pulseseek_decoder_symphonia::waveform::WaveformExtractionWorker;
use pulseseek_domain::decoder::{DecodeError, Decoder, ProbeResult, StreamMetadata};
use pulseseek_domain::playback::position::{Duration, Position, SeekTarget};
use pulseseek_domain::waveform::extraction::{
    extract_sampled_overview, ExtractionError, ExtractionOptions, WindowRequest,
};
use pulseseek_domain::waveform::peak::Peak;

const SILENT_STEREO_WAV: &[u8] = include_bytes!("fixtures/silent-stereo-44100.wav");
const SILENT_STEREO_FLAC: &[u8] = include_bytes!("fixtures/silent-stereo-44100.flac");
const SILENT_STEREO_MP3: &[u8] = include_bytes!("fixtures/silent-stereo-44100.mp3");
const SINE_STEREO_FLAC: &[u8] = include_bytes!("fixtures/sine-stereo-44100.flac");
const SINE_STEREO_MP3: &[u8] = include_bytes!("fixtures/sine-stereo-44100.mp3");

/// Write a PCM WAV file (16-bit) from f32 samples (interleaved).
fn write_pcm_wav(path: &Path, channels: u16, sample_rate: u32, samples: &[f32]) {
    let bits_per_sample: u16 = 16;
    let block_align = channels * (bits_per_sample / 8);
    let byte_rate = sample_rate * block_align as u32;
    let data_size = samples.len() as u32 * (bits_per_sample / 8) as u32;
    let file_size = 36 + data_size;

    let mut f = File::create(path).unwrap();
    f.write_all(b"RIFF").unwrap();
    f.write_all(&file_size.to_le_bytes()).unwrap();
    f.write_all(b"WAVE").unwrap();
    f.write_all(b"fmt ").unwrap();
    f.write_all(&16u32.to_le_bytes()).unwrap();
    f.write_all(&1u16.to_le_bytes()).unwrap(); // PCM
    f.write_all(&channels.to_le_bytes()).unwrap();
    f.write_all(&sample_rate.to_le_bytes()).unwrap();
    f.write_all(&byte_rate.to_le_bytes()).unwrap();
    f.write_all(&block_align.to_le_bytes()).unwrap();
    f.write_all(&bits_per_sample.to_le_bytes()).unwrap();
    f.write_all(b"data").unwrap();
    f.write_all(&data_size.to_le_bytes()).unwrap();
    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let sample_i16 = (clamped * i16::MAX as f32) as i16;
        f.write_all(&sample_i16.to_le_bytes()).unwrap();
    }
}

fn write_fixture(dir: &tempfile::TempDir, name: &str, data: &[u8]) -> std::path::PathBuf {
    let path = dir.path().join(name);
    let mut f = File::create(&path).unwrap();
    f.write_all(data).unwrap();
    path
}

fn options(target_peak_count: u64) -> ExtractionOptions {
    ExtractionOptions::new(target_peak_count, 8).expect("valid options")
}

fn assert_peaks_within_bounds(peaks: &[Peak]) {
    for p in peaks {
        assert!(p.min() >= -1.0 && p.min() <= p.max(), "invalid peak {p:?}");
        assert!(p.max() <= 1.0, "invalid peak {p:?}");
    }
}

// ── Overview worker ────────────────────────────────────────────────

#[test]
fn worker_overview_generates_bounded_pyramid() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.wav");
    // 440 Hz sine, exactly 8000 frames at 44.1 kHz.
    let samples: Vec<f32> = (0..8000)
        .map(|i| ((i as f32) * 2.0 * std::f32::consts::PI * 440.0 / 44100.0).sin())
        .collect();
    write_pcm_wav(&path, 1, 44100, &samples);

    let worker = WaveformExtractionWorker::start_overview_from_path(&path, options(100))
        .expect("worker starts");
    let w = worker.wait().expect("extraction succeeds");
    assert_eq!(w.channels(), 1);
    assert_eq!(w.finest().samples_per_peak, 80);
    assert_eq!(w.finest().peaks.len(), 100);
    assert!(!w.levels().is_empty());
    for level in w.levels() {
        assert_peaks_within_bounds(&level.peaks);
    }
}

#[test]
fn worker_overview_silent_fixture_produces_zero_peaks() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "silent.wav", SILENT_STEREO_WAV);

    let worker = WaveformExtractionWorker::start_overview_from_path(&path, options(1000))
        .expect("worker starts");
    let w = worker.wait().expect("extraction succeeds");
    assert_eq!(w.channels(), 2);
    assert!(!w.levels().is_empty());
    for peak in &w.finest().peaks {
        assert_eq!(peak.min(), 0.0);
        assert_eq!(peak.max(), 0.0);
    }
}

#[test]
fn sampled_preview_supports_every_audio_player_format() {
    let dir = tempfile::tempdir().unwrap();
    for (name, bytes) in [
        ("silent.wav", SILENT_STEREO_WAV),
        ("silent.flac", SILENT_STEREO_FLAC),
        ("silent.mp3", SILENT_STEREO_MP3),
    ] {
        let path = write_fixture(&dir, name, bytes);
        let mut decoder = DecoderRegistry::open(&path).expect("decoder opens");
        let waveform = extract_sampled_overview(&mut *decoder, 8, &|| false)
            .expect("sampled preview succeeds");

        assert_eq!(waveform.channels(), 2, "{name}");
        assert_eq!(waveform.finest().peaks.len(), 16, "{name}");
    }
}

#[test]
fn sampled_preview_keeps_non_silent_compressed_audio_visible() {
    let dir = tempfile::tempdir().unwrap();
    for (name, bytes) in [("sine.flac", SINE_STEREO_FLAC), ("sine.mp3", SINE_STEREO_MP3)] {
        let path = write_fixture(&dir, name, bytes);
        let mut decoder = DecoderRegistry::open(&path).expect("decoder opens");
        let waveform = extract_sampled_overview(&mut *decoder, 64, &|| false)
            .expect("sampled preview succeeds");

        assert!(
            waveform.finest().peaks.iter().any(|peak| peak.min() < -0.01 || peak.max() > 0.01),
            "{name} should not render as a flat line"
        );
    }
}

#[test]
fn worker_window_sample_level_precision() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("exact.wav");
    // Exact 16-bit round-trip values: 1.0, -1.0, 0.0 repeating. A 1 kHz
    // source makes the 3 ms window cover exactly 3 samples.
    let samples: Vec<f32> = (0..64)
        .map(|i| match i % 3 {
            0 => 1.0,
            1 => -1.0,
            _ => 0.0,
        })
        .collect();
    write_pcm_wav(&path, 1, 1000, &samples);

    let request = WindowRequest::new(0, 3, 1).expect("valid window");
    let worker =
        WaveformExtractionWorker::start_window_from_path(&path, request).expect("worker starts");
    let peaks = worker.wait().expect("extraction succeeds");
    assert_eq!(peaks.len(), 3);
    // 16-bit PCM quantization maps 1.0 to 32767/32768 ~= 0.99997.
    assert!((peaks[0].min() - 1.0).abs() < 1e-3, "sample 0 mismatch");
    assert!((peaks[1].min() + 1.0).abs() < 1e-3, "sample 1 mismatch");
    assert_eq!(peaks[2].min(), 0.0);
    for p in &peaks {
        assert_eq!(p.min(), p.max());
    }
}

// ── Empty and corrupt files ────────────────────────────────────────

#[test]
fn worker_rejects_empty_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty.wav");
    File::create(&path).unwrap();

    let result = WaveformExtractionWorker::start_overview_from_path(&path, options(100));
    assert!(result.is_err(), "empty file should be rejected");
}

#[test]
fn worker_rejects_corrupt_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "corrupt.wav", b"this is not an audio file");

    let result = WaveformExtractionWorker::start_overview_from_path(&path, options(100));
    assert!(result.is_err(), "corrupt file should be rejected");
}

// ── Cancellation ───────────────────────────────────────────────────

/// A decoder whose reads block until released, for deterministic cancellation.
struct BlockingDecoder {
    release: mpsc::Receiver<()>,
    data: Vec<f32>,
    position: usize,
    channels: u16,
    sample_rate: u32,
}

impl Decoder for BlockingDecoder {
    fn probe(&self) -> ProbeResult {
        ProbeResult::Supported
    }

    fn metadata(&mut self) -> Result<StreamMetadata, DecodeError> {
        Ok(StreamMetadata {
            sample_rate: self.sample_rate,
            channels: self.channels,
            duration: Duration::from_millis(1000),
            bit_depth: None,
            codec: "test",
        })
    }

    fn read(&mut self, buf: &mut [f32]) -> Result<usize, DecodeError> {
        let _ = self.release.recv(); // block until the test releases us
        let to_copy = buf.len().min(self.data.len() - self.position);
        buf[..to_copy].copy_from_slice(&self.data[self.position..self.position + to_copy]);
        self.position += to_copy;
        Ok(to_copy)
    }

    fn seek(&mut self, target: SeekTarget) -> Result<Position, DecodeError> {
        self.position = (target.position().as_millis() * self.sample_rate as u64 / 1000
            * self.channels as u64) as usize;
        Ok(Position::from_millis(target.position().as_millis()))
    }
}

#[test]
fn worker_cancellation_is_deterministic() {
    let (release_tx, release_rx) = mpsc::channel();
    let decoder = BlockingDecoder {
        release: release_rx,
        data: vec![0.0; 100_000],
        position: 0,
        channels: 1,
        sample_rate: 1000,
    };

    let worker = WaveformExtractionWorker::start_overview(Box::new(decoder), options(100));
    worker.cancel();
    let _ = release_tx.send(());
    let err = worker.wait().expect_err("cancelled extraction");
    assert_eq!(err, ExtractionError::Cancelled);
}
