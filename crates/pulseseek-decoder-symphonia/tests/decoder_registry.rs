use std::io::Write;

use pulseseek_decoder_symphonia::registry::DecoderRegistry;

fn write_fixture(dir: &tempfile::TempDir, name: &str, data: &[u8]) -> std::path::PathBuf {
    let path = dir.path().join(name);
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(data).unwrap();
    path
}

const WAV_FIXTURE: &[u8] = include_bytes!("fixtures/silent-stereo-44100.wav");
const FLAC_FIXTURE: &[u8] = include_bytes!("fixtures/silent-stereo-44100.flac");
const MP3_FIXTURE: &[u8] = include_bytes!("fixtures/silent-stereo-44100.mp3");

#[test]
fn registry_opens_wav() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "test.wav", WAV_FIXTURE);
    let mut decoder = DecoderRegistry::open(&path).unwrap();
    let meta = decoder.metadata().unwrap();
    assert_eq!(meta.channels, 2);
}

#[test]
fn registry_opens_flac() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "test.flac", FLAC_FIXTURE);
    let mut decoder = DecoderRegistry::open(&path).unwrap();
    let meta = decoder.metadata().unwrap();
    assert_eq!(meta.channels, 2);
}

#[test]
fn registry_opens_mp3() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "test.mp3", MP3_FIXTURE);
    let mut decoder = DecoderRegistry::open(&path).unwrap();
    let meta = decoder.metadata().unwrap();
    assert_eq!(meta.channels, 2);
}

#[test]
fn registry_opens_wav_as_mp3_extension() {
    let dir = tempfile::tempdir().unwrap();
    // WAV content with .mp3 extension — probe must detect WAV.
    let path = write_fixture(&dir, "sound.mp3", WAV_FIXTURE);
    let mut decoder = DecoderRegistry::open(&path).unwrap();
    let meta = decoder.metadata().unwrap();
    assert_eq!(meta.sample_rate, 44100);
    assert_eq!(meta.channels, 2);
}

#[test]
fn registry_opens_flac_as_wav_extension() {
    let dir = tempfile::tempdir().unwrap();
    // FLAC content with .wav extension — probe must detect FLAC.
    let path = write_fixture(&dir, "audio.wav", FLAC_FIXTURE);
    let mut decoder = DecoderRegistry::open(&path).unwrap();
    let meta = decoder.metadata().unwrap();
    assert_eq!(meta.sample_rate, 44100);
    assert_eq!(meta.channels, 2);
}

#[test]
fn registry_rejects_corrupt() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("corrupt.wav");
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(b"not any audio file").unwrap();
    drop(f);

    let result = DecoderRegistry::open(&path);
    assert!(result.is_err(), "corrupt file should be rejected");
}

#[test]
fn registry_rejects_unsupported() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("readme.flac");
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(b"This is not an audio file.").unwrap();
    drop(f);

    let result = DecoderRegistry::open(&path);
    assert!(result.is_err(), "unsupported file should be rejected");
}
