use std::io::Write;

use pulseseek_decoder_symphonia::FlacDecoder;
use pulseseek_domain::decoder::{Decoder, ProbeResult};
use pulseseek_domain::playback::position::Position;

/// Write embedded fixture bytes to a temp file and return its path.
fn write_fixture(dir: &tempfile::TempDir, name: &str, data: &[u8]) -> std::path::PathBuf {
    let path = dir.path().join(name);
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(data).unwrap();
    path
}

const FIXTURE_SILENT_MONO_22050: &[u8] = include_bytes!("fixtures/silent-mono-22050.flac");

const FIXTURE_SILENT_MONO_44100: &[u8] = include_bytes!("fixtures/silent-mono-44100.flac");

const FIXTURE_SILENT_STEREO_44100: &[u8] = include_bytes!("fixtures/silent-stereo-44100.flac");

const FIXTURE_SINE_STEREO_44100: &[u8] = include_bytes!("fixtures/sine-stereo-44100.flac");

#[test]
fn valid_flac_probe_supported() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "test.flac", FIXTURE_SILENT_MONO_44100);
    let decoder = FlacDecoder::open(&path).unwrap();
    assert_eq!(decoder.probe(), ProbeResult::Supported);
}

#[test]
fn valid_flac_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "test.flac", FIXTURE_SILENT_MONO_44100);
    let mut decoder = FlacDecoder::open(&path).unwrap();
    let meta = decoder.metadata().unwrap();
    assert_eq!(meta.sample_rate, 44100);
    assert_eq!(meta.channels, 1);
    assert!(meta.bit_depth.is_some(), "FLAC should have bit depth");
    assert_eq!(meta.codec, "FLAC");
}

#[test]
fn valid_flac_read_frames() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "test.flac", FIXTURE_SILENT_MONO_44100);
    let mut decoder = FlacDecoder::open(&path).unwrap();
    let mut buf = vec![0.0f32; 44100];
    let frames = decoder.read(&mut buf).unwrap();
    assert_eq!(frames, 44100);
}

#[test]
fn mono_flac_decodes_correctly() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "test.flac", FIXTURE_SILENT_MONO_22050);
    let mut decoder = FlacDecoder::open(&path).unwrap();
    let meta = decoder.metadata().unwrap();
    assert_eq!(meta.channels, 1);
    assert_eq!(meta.sample_rate, 22050);
}

#[test]
fn stereo_flac_decodes_correctly() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "test.flac", FIXTURE_SILENT_STEREO_44100);
    let mut decoder = FlacDecoder::open(&path).unwrap();
    let meta = decoder.metadata().unwrap();
    assert_eq!(meta.channels, 2);
    assert_eq!(meta.sample_rate, 44100);
}

#[test]
fn seek_then_read_returns_different_data() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "test.flac", FIXTURE_SINE_STEREO_44100);
    let mut decoder = FlacDecoder::open(&path).unwrap();
    let meta = decoder.metadata().unwrap();

    // Read first 100 frames without seek.
    let mut buf_start = vec![0.0f32; 100];
    decoder.read(&mut buf_start).unwrap();

    // Seek to 501ms — past the initial read region (ends ~2.3ms)
    // and at a non-integer cycle count for 440Hz (440*0.501 = 220.44).
    // Most FLAC encoders place a seekpoint in this range for a 2s file.
    let dur = meta.duration;
    let target = dur.seek_to(Position::from_millis(501)).unwrap();
    let pos = decoder.seek(target).unwrap();
    assert_eq!(pos.as_millis(), 501);

    // Read after seek.
    let mut buf_seek = vec![0.0f32; 100];
    decoder.read(&mut buf_seek).unwrap();

    // Sine tone at 440Hz: sample values at start and midpoint differ.
    let same = buf_start.iter().zip(buf_seek.iter()).all(|(a, b)| (a - b).abs() < 0.001);
    assert!(!same, "data after seek should differ from start");
}

#[test]
fn corrupt_flac_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("corrupt.flac");
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(b"not a flac file").unwrap();
    drop(f);

    let result = FlacDecoder::open(&path);
    assert!(result.is_err(), "corrupt file should produce error");
}
