use std::path::Path;

use pulseseek_domain::browser::probe::{ProbeFile, ProbeResult};
use tempfile::tempdir;

use super::NativeProbe;

/// Writes a decoder fixture into `path` by embedding the real audio bytes so
/// the probe exercises actual decoding rather than a synthetic header.
fn write_fixture(path: &Path, fixture: &[u8]) {
    std::fs::write(path, fixture).expect("write fixture");
}

const WAV: &[u8] =
    include_bytes!("../../../pulseseek-decoder-symphonia/tests/fixtures/silent-stereo-44100.wav");
const AIFF: &[u8] =
    include_bytes!("../../../pulseseek-decoder-symphonia/tests/fixtures/sine-stereo-44100.aiff");
const FLAC: &[u8] =
    include_bytes!("../../../pulseseek-decoder-symphonia/tests/fixtures/silent-stereo-44100.flac");
const MP3: &[u8] =
    include_bytes!("../../../pulseseek-decoder-symphonia/tests/fixtures/silent-stereo-44100.mp3");
const OGG: &[u8] =
    include_bytes!("../../../pulseseek-decoder-symphonia/tests/fixtures/sine-stereo-44100.ogg");
const M4A_AAC: &[u8] =
    include_bytes!("../../../pulseseek-decoder-symphonia/tests/fixtures/sine-stereo-44100.m4a");
const M4A_ALAC: &[u8] = include_bytes!(
    "../../../pulseseek-decoder-symphonia/tests/fixtures/sine-stereo-44100-alac.m4a"
);

fn classify(path: &Path) -> ProbeResult {
    NativeProbe.probe(path).expect("probe should succeed")
}

#[test]
fn probe_classifies_directory() {
    let dir = tempdir().expect("create temp dir");
    assert_eq!(classify(dir.path()), ProbeResult::Directory);
}

#[test]
fn probe_classifies_playable_wav() {
    let dir = tempdir().expect("create temp dir");
    let path = dir.path().join("song.wav");
    write_fixture(&path, WAV);
    assert_eq!(classify(&path), ProbeResult::Playable);
}

#[test]
fn probe_classifies_playable_aiff() {
    let dir = tempdir().expect("create temp dir");
    let path = dir.path().join("song.aiff");
    write_fixture(&path, AIFF);
    assert_eq!(classify(&path), ProbeResult::Playable);
}

#[test]
fn probe_classifies_playable_flac() {
    let dir = tempdir().expect("create temp dir");
    let path = dir.path().join("song.flac");
    write_fixture(&path, FLAC);
    assert_eq!(classify(&path), ProbeResult::Playable);
}

#[test]
fn probe_classifies_playable_mp3() {
    let dir = tempdir().expect("create temp dir");
    let path = dir.path().join("song.mp3");
    write_fixture(&path, MP3);
    assert_eq!(classify(&path), ProbeResult::Playable);
}

#[test]
fn probe_classifies_playable_ogg() {
    let dir = tempdir().expect("create temp dir");
    let path = dir.path().join("song.ogg");
    write_fixture(&path, OGG);
    assert_eq!(classify(&path), ProbeResult::Playable);
}

#[test]
fn probe_accepts_oga_alias() {
    let dir = tempdir().expect("create temp dir");
    let path = dir.path().join("song.oga");
    write_fixture(&path, OGG);
    assert_eq!(classify(&path), ProbeResult::Playable);
}

#[test]
fn probe_classifies_playable_m4a_aac() {
    let dir = tempdir().expect("create temp dir");
    let path = dir.path().join("song.m4a");
    write_fixture(&path, M4A_AAC);
    assert_eq!(classify(&path), ProbeResult::Playable);
}

#[test]
fn probe_classifies_playable_m4a_alac() {
    let dir = tempdir().expect("create temp dir");
    let path = dir.path().join("song.m4a");
    write_fixture(&path, M4A_ALAC);
    assert_eq!(classify(&path), ProbeResult::Playable);
}

#[test]
fn probe_accepts_wave_alias() {
    let dir = tempdir().expect("create temp dir");
    let path = dir.path().join("song.wave");
    write_fixture(&path, WAV);
    assert_eq!(classify(&path), ProbeResult::Playable);
}

#[test]
fn probe_accepts_aif_alias() {
    let dir = tempdir().expect("create temp dir");
    let path = dir.path().join("song.aif");
    write_fixture(&path, AIFF);
    assert_eq!(classify(&path), ProbeResult::Playable);
}

#[test]
fn probe_classifies_uppercase_extension() {
    let dir = tempdir().expect("create temp dir");
    let path = dir.path().join("SONG.WAV");
    write_fixture(&path, WAV);
    assert_eq!(classify(&path), ProbeResult::Playable);
}

#[test]
fn probe_classifies_unsupported_extension_without_probing() {
    let dir = tempdir().expect("create temp dir");
    let path = dir.path().join("notes.txt");
    std::fs::write(&path, "hello").expect("write notes");
    assert_eq!(classify(&path), ProbeResult::Unsupported);
}

#[test]
fn probe_classifies_unsupported_binary_extension() {
    let dir = tempdir().expect("create temp dir");
    let path = dir.path().join("image.png");
    std::fs::write(&path, [0x89, 0x50, 0x4e, 0x47]).expect("write image");
    assert_eq!(classify(&path), ProbeResult::Unsupported);
}

#[test]
fn probe_classifies_corrupt_audio_as_unsupported() {
    let dir = tempdir().expect("create temp dir");
    let path = dir.path().join("corrupt.wav");
    std::fs::write(&path, b"not a real wav file").expect("write corrupt");
    assert_eq!(classify(&path), ProbeResult::Unsupported);
}

#[test]
fn probe_classifies_missing_path() {
    let dir = tempdir().expect("create temp dir");
    let path = dir.path().join("gone.mp3");
    assert_eq!(classify(&path), ProbeResult::Missing);
}

#[test]
fn probe_classifies_extensionless_file_as_unsupported() {
    let dir = tempdir().expect("create temp dir");
    let path = dir.path().join("README");
    std::fs::write(&path, "readme").expect("write file");
    assert_eq!(classify(&path), ProbeResult::Unsupported);
}
