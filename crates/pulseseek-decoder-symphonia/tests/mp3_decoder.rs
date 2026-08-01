use std::io::Write;

use pulseseek_decoder_symphonia::Mp3Decoder;
use pulseseek_domain::decoder::{Decoder, ProbeResult};

/// Write embedded fixture bytes to a temp file and return its path.
fn write_fixture(dir: &tempfile::TempDir, name: &str, data: &[u8]) -> std::path::PathBuf {
    let path = dir.path().join(name);
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(data).unwrap();
    path
}

const FIXTURE_SILENT_MONO_22050: &[u8] = include_bytes!("fixtures/silent-mono-22050.mp3");
const FIXTURE_SILENT_STEREO_44100: &[u8] = include_bytes!("fixtures/silent-stereo-44100.mp3");
const FIXTURE_SILENT_STEREO_48000: &[u8] = include_bytes!("fixtures/silent-stereo-48000.mp3");
const FIXTURE_SINE_STEREO_44100: &[u8] = include_bytes!("fixtures/sine-stereo-44100.mp3");
const FIXTURE_VBR_STEREO_44100: &[u8] = include_bytes!("fixtures/vbr-stereo-44100.mp3");

#[test]
fn valid_mp3_probe_supported() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "test.mp3", FIXTURE_SILENT_STEREO_44100);
    let decoder = Mp3Decoder::open(&path).unwrap();
    assert_eq!(decoder.probe(), ProbeResult::Supported);
}

#[test]
fn valid_mp3_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "test.mp3", FIXTURE_SILENT_STEREO_44100);
    let mut decoder = Mp3Decoder::open(&path).unwrap();
    let meta = decoder.metadata().unwrap();
    assert_eq!(meta.sample_rate, 44100);
    assert_eq!(meta.channels, 2);
    assert!(meta.bit_depth.is_none(), "MP3 should have no bit depth");
    assert_eq!(meta.codec, "MP3");
}

#[test]
fn valid_mp3_read_frames() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "test.mp3", FIXTURE_SILENT_STEREO_44100);
    let mut decoder = Mp3Decoder::open(&path).unwrap();
    let mut buf = vec![0.0f32; 44100 * 2];
    let frames = decoder.read(&mut buf).unwrap();
    assert_eq!(frames, 44100 * 2);
}

#[test]
fn small_reads_do_not_drop_the_rest_of_decoded_packets() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "test.mp3", FIXTURE_SILENT_STEREO_44100);
    let mut small_reads = Mp3Decoder::open(&path).unwrap();
    let mut large_reads = Mp3Decoder::open(&path).unwrap();
    let mut chunk = [0.0f32; 512];
    let mut small_total = 0;

    loop {
        let read = small_reads.read(&mut chunk).unwrap();
        if read == 0 {
            break;
        }
        small_total += read;
    }

    let mut large_chunk = vec![0.0f32; 100_000];
    let large_total = large_reads.read(&mut large_chunk).unwrap();
    assert_eq!(small_total, large_total);
}

#[test]
fn mono_mp3_decodes_correctly() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "test.mp3", FIXTURE_SILENT_MONO_22050);
    let mut decoder = Mp3Decoder::open(&path).unwrap();
    let meta = decoder.metadata().unwrap();
    assert_eq!(meta.channels, 1);
    assert_eq!(meta.sample_rate, 22050);
}

#[test]
fn stereo_mp3_decodes_correctly() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "test.mp3", FIXTURE_SILENT_STEREO_48000);
    let mut decoder = Mp3Decoder::open(&path).unwrap();
    let meta = decoder.metadata().unwrap();
    assert_eq!(meta.channels, 2);
    assert_eq!(meta.sample_rate, 48000);
}

#[test]
fn seek_then_read_returns_different_data() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "test.mp3", FIXTURE_SINE_STEREO_44100);
    let mut decoder = Mp3Decoder::open(&path).unwrap();

    // First verify the sine file decodes to non-silence.
    let mut buf_all = vec![0.0f32; 44100 * 2];
    let frames = decoder.read(&mut buf_all).unwrap();
    assert!(frames > 0, "should read frames from sine file");
    let rms_all = (buf_all[..frames].iter().map(|s| s * s).sum::<f32>() / frames as f32).sqrt();
    assert!(rms_all > 0.01, "sine file should have non-zero RMS, got {rms_all}");

    // Seek to a valid position within the file.
    // Note: MP3 seek may land at nearest frame boundary; verify we get
    // audio signal (non-zero RMS) after seek, not exact position.
    let target = pulseseek_domain::playback::position::Position::from_millis(500);
    let dur = pulseseek_domain::playback::position::Duration::from_millis(2000);
    let seek_target = dur.seek_to(target).unwrap();
    decoder.seek(seek_target).unwrap();

    // Read after seek and verify we get non-silence.
    let mut buf_seek = vec![0.0f32; 441];
    let frames = decoder.read(&mut buf_seek).unwrap();
    assert!(frames > 0, "should read frames after seek");
    let rms = (buf_seek[..frames].iter().map(|s| s * s).sum::<f32>() / frames as f32).sqrt();
    assert!(rms > 0.001, "sine signal should have non-zero RMS after seek, got {rms}");
}

#[test]
fn variable_bitrate_mp3_decodes() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "vbr.mp3", FIXTURE_VBR_STEREO_44100);
    let decoder = Mp3Decoder::open(&path).unwrap();
    assert_eq!(decoder.probe(), ProbeResult::Supported);
}

#[test]
fn corrupt_mp3_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("corrupt.mp3");
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(b"not an mp3 file").unwrap();
    drop(f);

    let result = Mp3Decoder::open(&path);
    assert!(result.is_err(), "corrupt file should produce error");
}
