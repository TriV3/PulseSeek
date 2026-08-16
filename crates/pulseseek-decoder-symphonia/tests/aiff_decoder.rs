use std::io::Write;

use pulseseek_decoder_symphonia::AiffDecoder;
use pulseseek_domain::decoder::{Decoder, ProbeResult};
use pulseseek_domain::playback::position::{Duration, Position};

fn write_fixture(dir: &tempfile::TempDir, name: &str, data: &[u8]) -> std::path::PathBuf {
    let path = dir.path().join(name);
    std::fs::File::create(&path).unwrap().write_all(data).unwrap();
    path
}

const FIXTURE: &[u8] = include_bytes!("fixtures/sine-stereo-44100.aiff");

#[test]
fn valid_aiff_probe_and_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "test.aiff", FIXTURE);
    let mut decoder = AiffDecoder::open(&path).unwrap();

    assert_eq!(decoder.probe(), ProbeResult::Supported);
    let metadata = decoder.metadata().unwrap();
    assert_eq!(metadata.sample_rate, 44_100);
    assert_eq!(metadata.channels, 2);
    assert_eq!(metadata.codec, "PCM");
}

#[test]
fn valid_aiff_reads_and_seeks() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "test.aif", FIXTURE);
    let mut decoder = AiffDecoder::open(&path).unwrap();
    let mut samples = vec![0.0; 8820];

    assert_eq!(decoder.read(&mut samples).unwrap(), samples.len());
    let target = Duration::Unknown.seek_to(Position::from_millis(500)).unwrap();
    assert_eq!(decoder.seek(target).unwrap().as_millis(), 500);
}

#[test]
fn corrupt_aiff_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "corrupt.aiff", b"not an AIFF file");
    assert!(AiffDecoder::open(&path).is_err());
}
