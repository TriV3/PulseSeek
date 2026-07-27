use std::fs::File;
use std::io::Write;
use std::path::Path;

use pulseseek_decoder_symphonia::WavDecoder;
use pulseseek_domain::decoder::{Decoder, ProbeResult};
use pulseseek_domain::playback::position::{Duration, Position};

/// Write a WAV file with PCM format.
fn write_pcm_wav(path: &Path, channels: u16, sample_rate: u32, samples: &[f32], format_code: u16) {
    let bits_per_sample: u16 = 16;
    let block_align = channels * (bits_per_sample / 8);
    let byte_rate = sample_rate * block_align as u32;
    let data_size = samples.len() as u32 * (bits_per_sample / 8) as u32;
    let file_size = 36 + data_size;

    let mut f = File::create(path).unwrap();

    // RIFF header
    f.write_all(b"RIFF").unwrap();
    f.write_all(&file_size.to_le_bytes()).unwrap();
    f.write_all(b"WAVE").unwrap();

    // fmt chunk
    f.write_all(b"fmt ").unwrap();
    f.write_all(&16u32.to_le_bytes()).unwrap(); // chunk size
    f.write_all(&format_code.to_le_bytes()).unwrap(); // audio format (1=PCM)
    f.write_all(&channels.to_le_bytes()).unwrap();
    f.write_all(&sample_rate.to_le_bytes()).unwrap();
    f.write_all(&byte_rate.to_le_bytes()).unwrap();
    f.write_all(&block_align.to_le_bytes()).unwrap();
    f.write_all(&bits_per_sample.to_le_bytes()).unwrap();

    // data chunk
    f.write_all(b"data").unwrap();
    f.write_all(&data_size.to_le_bytes()).unwrap();

    // PCM data (convert f32 to i16)
    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let sample_i16 = (clamped * i16::MAX as f32) as i16;
        f.write_all(&sample_i16.to_le_bytes()).unwrap();
    }
}

fn write_valid_wav(path: &Path, channels: u16, sample_rate: u32, samples: &[f32]) {
    write_pcm_wav(path, channels, sample_rate, samples, 1); // 1 = PCM
}

/// Generate a ramp of f32 values.
fn ramp(len: usize) -> Vec<f32> {
    (0..len).map(|i| (i % 1000) as f32 / 1000.0).collect()
}

// ── Tests ──────────────────────────────────────────────────────────────

#[test]
fn valid_pcm_wav_probe_supported() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.wav");
    write_valid_wav(&path, 2, 44100, &[0.0; 44100 * 2]);

    let decoder = WavDecoder::open(&path).unwrap();
    assert_eq!(decoder.probe(), ProbeResult::Supported);
}

#[test]
fn valid_pcm_wav_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.wav");
    let samples = ramp(48000);
    write_valid_wav(&path, 1, 48000, &samples);

    let mut decoder = WavDecoder::open(&path).unwrap();
    let meta = decoder.metadata().unwrap();
    assert_eq!(meta.sample_rate, 48000);
    assert_eq!(meta.channels, 1);
    assert!(meta.bit_depth.is_some(), "WAV should have bit depth");
    assert_eq!(meta.codec, "PCM");
}

#[test]
fn valid_pcm_wav_read_frames() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.wav");
    let num_frames = 44100 * 2; // 1s stereo
    let samples = vec![0.0; num_frames];
    write_valid_wav(&path, 2, 44100, &samples);

    let mut decoder = WavDecoder::open(&path).unwrap();
    let mut buf = vec![0.0f32; num_frames];
    let frames = decoder.read(&mut buf).unwrap();
    assert_eq!(frames, num_frames);
}

#[test]
fn valid_pcm_wav_read_values() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.wav");
    let samples = ramp(100);
    write_valid_wav(&path, 1, 44100, &samples);

    let mut decoder = WavDecoder::open(&path).unwrap();
    let mut buf = vec![0.0f32; 100];
    let frames = decoder.read(&mut buf).unwrap();
    assert_eq!(frames, 100);
    for i in 0..10 {
        let expected = samples[i];
        let actual = buf[i];
        let diff = (expected - actual).abs();
        assert!(diff < 0.01, "mismatch at {i}: expected {expected}, got {actual}");
    }
}

#[test]
fn mono_wav_decodes_correctly() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.wav");
    write_valid_wav(&path, 1, 22050, &[0.0; 22050]);

    let mut decoder = WavDecoder::open(&path).unwrap();
    let meta = decoder.metadata().unwrap();
    assert_eq!(meta.channels, 1);
    assert_eq!(meta.sample_rate, 22050);
}

#[test]
fn stereo_wav_decodes_correctly() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.wav");
    write_valid_wav(&path, 2, 96000, &[0.0; 96000 * 2]);

    let mut decoder = WavDecoder::open(&path).unwrap();
    let meta = decoder.metadata().unwrap();
    assert_eq!(meta.channels, 2);
    assert_eq!(meta.sample_rate, 96000);
}

#[test]
fn unsupported_format_probe_not_supported() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.wav");
    // Format code 0x0003 = Microsoft ADPCM — not PCM, Symphonia can't decode it.
    write_pcm_wav(&path, 2, 44100, &[0.0; 100], 0x0003);

    let result = WavDecoder::open(&path);
    assert!(result.is_err(), "unsupported WAV codec should produce error");
}

#[test]
fn completely_corrupt_header_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("corrupt.wav");
    // Write garbage
    let mut f = File::create(&path).unwrap();
    f.write_all(b"this is not a wav file").unwrap();
    drop(f);

    let result = WavDecoder::open(&path);
    assert!(result.is_err(), "corrupt file should produce error");
}

#[test]
fn seek_then_read_changes_position() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.wav");
    let sample_rate = 1000u32; // 1 sample per ms
    let channels = 1u16;
    let duration_ms = 100u64;
    let num_samples = (sample_rate as usize) * (duration_ms as usize) * (channels as usize);
    let samples: Vec<f32> = (0..num_samples).map(|i| i as f32 / num_samples as f32).collect();
    write_valid_wav(&path, channels, sample_rate, &samples);

    let mut decoder = WavDecoder::open(&path).unwrap();

    // Seek to 50ms
    let dur = Duration::from_millis(duration_ms);
    let target = dur.seek_to(Position::from_millis(50)).unwrap();
    let pos = decoder.seek(target).unwrap();
    assert_eq!(pos.as_millis(), 50);

    // Read after seek — should start from position 50
    let mut buf = vec![0.0f32; 10];
    let frames = decoder.read(&mut buf).unwrap();
    assert_eq!(frames, 10);

    // At position 50 (sample 50), the ramp value should be ~0.05
    let expected_at_50 = 50.0 / num_samples as f32;
    let diff = (buf[0] - expected_at_50).abs();
    assert!(diff < 0.01, "expected {expected_at_50} at position 50, got {}", buf[0]);
}
