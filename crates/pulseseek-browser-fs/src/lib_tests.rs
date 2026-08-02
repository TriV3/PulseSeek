use std::fs;
use std::path::Path;

use super::*;
use pulseseek_domain::error::ErrorContract;

fn create_dir(path: &Path) {
    fs::create_dir_all(path).expect("create test dir");
}

fn create_file(path: &Path) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dir");
    }
    fs::write(path, b"content").expect("create test file");
}

fn create_wav(path: &Path) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dir");
    }
    fs::write(
        path,
        include_bytes!("../../pulseseek-decoder-symphonia/tests/fixtures/silent-stereo-44100.wav"),
    )
    .expect("create WAV fixture");
}

#[test]
fn native_reads_empty_folder() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let result = NativeFolderReader.read_folder(dir.path()).unwrap();
    assert!(result.is_empty(), "empty folder should return no entries");
}

#[test]
fn native_reads_folder_with_files() {
    let dir = tempfile::tempdir().expect("create temp dir");
    create_wav(&dir.path().join("a.wav"));
    create_wav(&dir.path().join("b.wav"));

    let result = NativeFolderReader.read_folder(dir.path()).unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn native_reads_file_and_audio_metadata() {
    let dir = tempfile::tempdir().expect("create temp dir");
    create_wav(&dir.path().join("audio.wav"));

    let result = NativeFolderReader.read_folder(dir.path()).unwrap();
    let BrowserEntry::PlayableFile(file) = &result[0] else {
        panic!("expected playable file");
    };
    let metadata = file.metadata.as_ref().expect("metadata should be available");

    assert!(metadata.duration_ms.is_some());
    assert!(metadata.size_bytes.is_some_and(|size| size > 0));
    assert!(metadata.modified_at_ms.is_some());
    assert_eq!(metadata.channels, Some(2));
    assert_eq!(metadata.sample_rate, Some(44_100));
    assert_eq!(metadata.bit_depth, Some(16));
    assert_eq!(metadata.codec.as_deref(), Some("PCM"));
}

#[test]
fn zero_stream_values_are_unavailable() {
    let metadata = playable_file_metadata(
        Some(pulseseek_domain::decoder::StreamMetadata {
            sample_rate: 0,
            channels: 0,
            duration: Duration::from_millis(0),
            bit_depth: None,
            codec: "MP3",
        }),
        None,
    );

    assert_eq!(metadata.duration_ms, None);
    assert_eq!(metadata.channels, None);
    assert_eq!(metadata.sample_rate, None);
    assert_eq!(metadata.codec.as_deref(), Some("MP3"));
}

#[test]
fn native_reads_folder_with_nested_folder() {
    let dir = tempfile::tempdir().expect("create temp dir");
    create_wav(&dir.path().join("root.wav"));
    create_dir(&dir.path().join("sub"));
    create_file(&dir.path().join("sub").join("nested.wav"));

    let result = NativeFolderReader.read_folder(dir.path()).unwrap();

    assert_eq!(result.len(), 2, "should only return direct children");

    let folder_count = result.iter().filter(|e| matches!(e, BrowserEntry::Folder(_))).count();
    let file_count = result.iter().filter(|e| matches!(e, BrowserEntry::PlayableFile(_))).count();
    assert_eq!(folder_count, 1, "should have one folder");
    assert_eq!(file_count, 1, "should have one file");
}

#[test]
fn native_reads_folder_with_symlink() {
    let dir = tempfile::tempdir().expect("create temp dir");
    create_wav(&dir.path().join("target.wav"));
    let link_path = dir.path().join("link.wav");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(dir.path().join("target.wav"), &link_path)
            .expect("create symlink");
    }
    #[cfg(windows)]
    {
        return;
    }

    let result = NativeFolderReader.read_folder(dir.path()).unwrap();
    let names: Vec<&str> = result.iter().map(|e| e.name()).collect();
    assert!(names.contains(&"link.wav"), "symlink should be listed");
    assert!(names.contains(&"target.wav"), "target should be listed");
}

#[test]
fn native_reads_folder_with_unicode_names() {
    let dir = tempfile::tempdir().expect("create temp dir");
    create_wav(&dir.path().join("über-jam.wav"));
    create_wav(&dir.path().join("alpha.wav"));

    let result = NativeFolderReader.read_folder(dir.path()).unwrap();
    let names: Vec<&str> = result.iter().map(|e| e.name()).collect();
    assert!(names.contains(&"über-jam.wav"));
    assert!(names.contains(&"alpha.wav"));
}

#[test]
fn native_returns_error_for_nonexistent_path() {
    let result = NativeFolderReader.read_folder(Path::new("/nonexistent/path/xyz"));
    assert!(result.is_err(), "nonexistent path should return error");
    let err = result.unwrap_err();
    assert_eq!(err.user_descriptor().category(), pulseseek_domain::error::ErrorCategory::NotFound);
}

#[test]
fn native_returns_error_for_file_path() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let file_path = dir.path().join("file.wav");
    create_file(&file_path);

    let result = NativeFolderReader.read_folder(&file_path);
    assert!(result.is_err(), "file path should return error");
}

#[test]
fn native_entries_are_sorted() {
    let dir = tempfile::tempdir().expect("create temp dir");
    create_dir(&dir.path().join("zzz_folder"));
    create_wav(&dir.path().join("aaa_file.wav"));
    create_dir(&dir.path().join("bbb_folder"));
    create_wav(&dir.path().join("ccc_file.wav"));

    let result = NativeFolderReader.read_folder(dir.path()).unwrap();
    let names: Vec<&str> = result.iter().map(|e| e.name()).collect();

    assert_eq!(names, vec!["bbb_folder", "zzz_folder", "aaa_file.wav", "ccc_file.wav"]);
}

#[test]
fn native_hides_unsupported_files_by_default() {
    let dir = tempfile::tempdir().expect("create temp dir");
    create_wav(&dir.path().join("audio.bin"));
    create_file(&dir.path().join("notes.txt"));

    let result = NativeFolderReader.read_folder(dir.path()).unwrap();

    assert!(result.is_empty());
}

#[test]
fn native_can_show_unsupported_files() {
    let dir = tempfile::tempdir().expect("create temp dir");
    create_wav(&dir.path().join("audio.txt"));
    create_file(&dir.path().join("notes.txt"));

    let result = NativeFolderReader.read_folder_with_options(dir.path(), true).unwrap();

    assert_eq!(result.len(), 2);
    assert!(result.iter().all(|entry| matches!(entry, BrowserEntry::UnsupportedFile(_))));
}

#[test]
fn native_rejects_corrupt_audio_as_unsupported() {
    let dir = tempfile::tempdir().expect("create temp dir");
    create_file(&dir.path().join("corrupt.wav"));

    let result = NativeFolderReader.read_folder_with_options(dir.path(), true).unwrap();

    assert!(matches!(result[0], BrowserEntry::UnsupportedFile(_)));
}

#[test]
fn preview_returns_audio_names_without_decoding_content() {
    let dir = tempfile::tempdir().unwrap();
    let audio_path = dir.path().join("large.wav");
    std::fs::write(&audio_path, b"not decoded during preview").unwrap();
    std::fs::create_dir(dir.path().join("Samples")).unwrap();

    let entries = NativeFolderReader.read_folder_preview(dir.path(), false).unwrap();

    assert_eq!(entries.len(), 2);
    assert!(entries.iter().any(|entry| entry.name() == "Samples"));
    assert!(entries.iter().any(|entry| entry.name() == "large.wav"));
}

#[test]
fn streamed_preview_emits_only_folders_before_audio_validation() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("Samples")).unwrap();
    std::fs::write(dir.path().join("first.mp3"), b"not decoded").unwrap();
    std::fs::write(dir.path().join("second.wav"), b"not decoded").unwrap();

    let mut chunks = Vec::new();
    NativeFolderReader
        .stream_folder_preview(
            dir.path(),
            false,
            2,
            || false,
            |entries| {
                chunks.push(entries.to_vec());
            },
        )
        .unwrap();

    assert_eq!(chunks.len(), 1);
    assert!(chunks.iter().all(|chunk| chunk.len() <= 2));
    let names: Vec<&str> = chunks.iter().flatten().map(|entry| entry.name()).collect();
    assert_eq!(names, vec!["Samples"]);
    assert!(chunks.iter().flatten().all(|entry| matches!(entry, BrowserEntry::Folder(_))));
}
