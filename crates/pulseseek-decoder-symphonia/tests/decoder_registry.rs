use std::io::Write;

use pulseseek_decoder_symphonia::{probe_stream_metadata, registry::DecoderRegistry};

fn write_fixture(dir: &tempfile::TempDir, name: &str, data: &[u8]) -> std::path::PathBuf {
    let path = dir.path().join(name);
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(data).unwrap();
    path
}

const WAV_FIXTURE: &[u8] = include_bytes!("fixtures/silent-stereo-44100.wav");
const FLAC_FIXTURE: &[u8] = include_bytes!("fixtures/silent-stereo-44100.flac");
const MP3_FIXTURE: &[u8] = include_bytes!("fixtures/silent-stereo-44100.mp3");
const AIFF_FIXTURE: &[u8] = include_bytes!("fixtures/sine-stereo-44100.aiff");
const OGG_FIXTURE: &[u8] = include_bytes!("fixtures/sine-stereo-44100.ogg");
const M4A_AAC_FIXTURE: &[u8] = include_bytes!("fixtures/sine-stereo-44100.m4a");
const M4A_ALAC_FIXTURE: &[u8] = include_bytes!("fixtures/sine-stereo-44100-alac.m4a");

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
fn registry_opens_aiff() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "test.aiff", AIFF_FIXTURE);
    let mut decoder = DecoderRegistry::open(&path).unwrap();
    let meta = decoder.metadata().unwrap();
    assert_eq!(meta.sample_rate, 44_100);
    assert_eq!(meta.channels, 2);
}

#[test]
fn registry_opens_ogg_vorbis() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "test.ogg", OGG_FIXTURE);
    let mut decoder = DecoderRegistry::open(&path).unwrap();
    let meta = decoder.metadata().unwrap();
    assert_eq!(meta.sample_rate, 44_100);
    assert_eq!(meta.channels, 2);
    assert_eq!(meta.codec, "Vorbis");
}

#[test]
fn registry_opens_m4a_aac() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "test.m4a", M4A_AAC_FIXTURE);
    let mut decoder = DecoderRegistry::open(&path).unwrap();
    let meta = decoder.metadata().unwrap();
    assert_eq!(meta.sample_rate, 44_100);
    assert_eq!(meta.channels, 2);
    assert_eq!(meta.codec, "AAC");
}

#[test]
fn registry_opens_m4a_alac() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "test.m4a", M4A_ALAC_FIXTURE);
    let mut decoder = DecoderRegistry::open(&path).unwrap();
    let meta = decoder.metadata().unwrap();
    assert_eq!(meta.sample_rate, 44_100);
    assert_eq!(meta.channels, 2);
    assert_eq!(meta.codec, "ALAC");
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

#[test]
fn lightweight_probe_reads_metadata_without_constructing_a_decoder() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "test.mp3", MP3_FIXTURE);

    let metadata = probe_stream_metadata(&path).expect("metadata probe");

    assert_eq!(metadata.channels, 2);
    assert_eq!(metadata.sample_rate, 44_100);
    assert_eq!(metadata.codec, "MP3");
}

#[test]
fn lightweight_probe_reads_ogg_vorbis_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "test.ogg", OGG_FIXTURE);

    let metadata = probe_stream_metadata(&path).expect("metadata probe");

    assert_eq!(metadata.channels, 2);
    assert_eq!(metadata.sample_rate, 44_100);
    assert_eq!(metadata.codec, "Vorbis");
}

#[test]
fn lightweight_probe_reads_m4a_aac_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "test.m4a", M4A_AAC_FIXTURE);

    let metadata = probe_stream_metadata(&path).expect("metadata probe");

    // isomp4/m4a containers do not expose the channel layout in the track's
    // codec parameters, so the lightweight probe reports channels as unknown
    // (0) without constructing a decoder. The full decoder path reports them.
    assert_eq!(metadata.sample_rate, 44_100);
    assert_eq!(metadata.codec, "AAC");
}

#[test]
fn lightweight_probe_rejects_a_misleading_audio_extension() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(&dir, "corrupt.wav", b"not any audio file");

    assert!(probe_stream_metadata(&path).is_err());
}
