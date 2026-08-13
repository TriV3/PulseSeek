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

fn create_aiff(path: &Path) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dir");
    }
    fs::write(
        path,
        include_bytes!("../../pulseseek-decoder-symphonia/tests/fixtures/sine-stereo-44100.aiff"),
    )
    .expect("create AIFF fixture");
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
fn native_reads_aiff_file_and_audio_metadata() {
    let dir = tempfile::tempdir().expect("create temp dir");
    create_aiff(&dir.path().join("audio.aif"));

    let result = NativeFolderReader.read_folder(dir.path()).unwrap();
    let BrowserEntry::PlayableFile(file) = &result[0] else {
        panic!("expected playable file");
    };
    let metadata = file.metadata.as_ref().expect("metadata should be available");

    assert_eq!(metadata.channels, Some(2));
    assert_eq!(metadata.sample_rate, Some(44_100));
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
fn streamed_preview_emits_folders_and_audio_candidates_without_metadata() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("Samples")).unwrap();
    std::fs::write(dir.path().join("first.mp3"), b"not decoded").unwrap();
    std::fs::write(dir.path().join("second.wav"), b"not decoded").unwrap();

    let mut chunks = Vec::new();
    NativeFolderReader
        .stream_folder_preview(
            dir.path(),
            false,
            false,
            2,
            || false,
            |entries| {
                chunks.push(entries.to_vec());
            },
        )
        .unwrap();

    assert_eq!(chunks.len(), 2);
    assert!(chunks.iter().all(|chunk| chunk.len() <= 2));
    let mut names: Vec<&str> = chunks.iter().flatten().map(|entry| entry.name()).collect();
    names.sort_unstable();
    assert_eq!(names, vec!["Samples", "first.mp3", "second.wav"]);
    assert!(chunks.iter().flatten().all(|entry| match entry {
        BrowserEntry::Folder(_) => true,
        BrowserEntry::PlayableFile(file) => file.metadata.is_none(),
        _ => false,
    }));
}

#[test]
fn streamed_preview_marks_leaf_folders_before_they_are_rendered() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("Empty")).unwrap();
    std::fs::create_dir_all(dir.path().join("Nested/Child")).unwrap();

    let mut entries = Vec::new();
    NativeFolderReader
        .stream_folder_preview(
            dir.path(),
            false,
            false,
            100,
            || false,
            |chunk| {
                entries.extend_from_slice(chunk);
            },
        )
        .unwrap();

    let folder_flags: Vec<_> = entries
        .iter()
        .filter_map(|entry| match entry {
            BrowserEntry::Folder(folder) => Some((folder.name.as_str(), folder.has_subfolders)),
            _ => None,
        })
        .collect();
    assert_eq!(folder_flags, vec![("Empty", Some(false)), ("Nested", Some(true))]);
}

#[test]
fn folder_preview_hides_dot_directories_unless_requested() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("Visible")).unwrap();
    std::fs::create_dir(dir.path().join(".hidden")).unwrap();

    let hidden = NativeFolderReader.read_folder_preview(dir.path(), false).unwrap();
    assert!(hidden.iter().any(|entry| entry.name() == "Visible"));
    assert!(hidden.iter().all(|entry| entry.name() != ".hidden"));

    let shown =
        NativeFolderReader.read_folder_preview_with_options(dir.path(), false, true).unwrap();
    assert!(shown.iter().any(|entry| entry.name() == ".hidden"));
}

fn collect_recursive(
    reader: &NativeFolderReader,
    path: &Path,
    show_unsupported: bool,
    batch_size: usize,
    is_cancelled: impl Fn() -> bool,
) -> Vec<BrowserEntry> {
    collect_recursive_with_hidden(reader, path, show_unsupported, false, batch_size, is_cancelled)
}

fn collect_recursive_with_hidden(
    reader: &NativeFolderReader,
    path: &Path,
    show_unsupported: bool,
    show_hidden: bool,
    batch_size: usize,
    is_cancelled: impl Fn() -> bool,
) -> Vec<BrowserEntry> {
    let mut entries = Vec::new();
    reader
        .stream_recursive_files(
            path,
            show_unsupported,
            show_hidden,
            batch_size,
            is_cancelled,
            |chunk| {
                entries.extend(chunk.iter().cloned());
            },
        )
        .expect("recursive stream should succeed");
    entries
}

#[test]
fn recursive_stream_skips_hidden_directories_unless_requested() {
    let dir = tempfile::tempdir().expect("create temp dir");
    create_wav(&dir.path().join("visible/visible.wav"));
    create_wav(&dir.path().join(".hidden/hidden.wav"));

    let hidden = collect_recursive(&NativeFolderReader, dir.path(), false, 100, || false);
    assert!(hidden.iter().any(|entry| entry.name() == "visible.wav"));
    assert!(hidden.iter().all(|entry| entry.name() != "hidden.wav"));

    let shown =
        collect_recursive_with_hidden(&NativeFolderReader, dir.path(), false, true, 100, || false);
    assert!(shown.iter().any(|entry| entry.name() == "hidden.wav"));
}

#[test]
fn recursive_stream_finds_playable_files_in_deep_tree() {
    let dir = tempfile::tempdir().expect("create temp dir");
    create_wav(&dir.path().join("a/b/c/real.wav"));
    create_file(&dir.path().join("a/notes.txt"));

    let entries = collect_recursive(&NativeFolderReader, dir.path(), false, 100, || false);

    let playable: Vec<&str> = entries
        .iter()
        .filter(|entry| matches!(entry, BrowserEntry::PlayableFile(_)))
        .map(|entry| entry.name())
        .collect();
    assert_eq!(playable, vec!["real.wav"], "nested playable file must be discovered");
    assert!(
        entries.iter().all(|entry| entry.id().as_str().ends_with("a/b/c/real.wav")),
        "entry id must carry the full path so recursive files are distinct",
    );
}

#[test]
fn recursive_stream_orders_subtrees_deterministically() {
    let dir = tempfile::tempdir().expect("create temp dir");
    create_wav(&dir.path().join("zzz/two.wav"));
    create_wav(&dir.path().join("aaa/one.wav"));
    create_wav(&dir.path().join("mid.wav"));

    let entries = collect_recursive(&NativeFolderReader, dir.path(), false, 100, || false);

    let names: Vec<&str> = entries.iter().map(|entry| entry.name()).collect();
    assert_eq!(
        names,
        vec!["mid.wav", "one.wav", "two.wav"],
        "recursive stream must traverse own files then sorted subtrees",
    );
}

#[test]
fn recursive_stream_sorts_files_within_each_directory() {
    let dir = tempfile::tempdir().expect("create temp dir");
    create_wav(&dir.path().join("root-b.wav"));
    create_wav(&dir.path().join("root-a.wav"));
    create_wav(&dir.path().join("sub/z.wav"));
    create_wav(&dir.path().join("sub/y.wav"));

    let entries = collect_recursive(&NativeFolderReader, dir.path(), false, 100, || false);

    let names: Vec<&str> = entries.iter().map(|entry| entry.name()).collect();
    assert_eq!(
        names,
        vec!["root-a.wav", "root-b.wav", "y.wav", "z.wav"],
        "files must be sorted within every directory, independent of read_dir order",
    );
}

#[cfg(unix)]
#[test]
fn recursive_stream_breaks_directory_symlink_cycle() {
    let dir = tempfile::tempdir().expect("create temp dir");
    create_wav(&dir.path().join("a/x.wav"));
    create_wav(&dir.path().join("y.wav"));
    std::os::unix::fs::symlink(dir.path(), dir.path().join("a/loop")).expect("create symlink");

    let entries = collect_recursive(&NativeFolderReader, dir.path(), false, 100, || false);

    let names: Vec<&str> = entries.iter().map(|entry| entry.name()).collect();
    assert_eq!(names.iter().filter(|name| **name == "y.wav").count(), 1);
    assert_eq!(names.iter().filter(|name| **name == "x.wav").count(), 1);
    assert!(
        entries.iter().all(|entry| !matches!(entry, BrowserEntry::Inaccessible(_))),
        "a symlink cycle must be skipped, not reported as an error",
    );
}

#[cfg(unix)]
#[test]
fn recursive_stream_reports_permission_boundary_and_continues() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("create temp dir");
    create_wav(&dir.path().join("ok.wav"));
    let blocked = dir.path().join("blocked");
    std::fs::create_dir(&blocked).expect("create blocked dir");
    std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o000))
        .expect("chmod blocked dir");

    let entries = collect_recursive(&NativeFolderReader, dir.path(), false, 100, || false);

    let _ = std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o755));

    assert!(
        entries.iter().any(|entry| entry.name() == "ok.wav"),
        "files outside the blocked boundary must still be streamed",
    );
    let BrowserEntry::Inaccessible(inaccessible) = entries
        .iter()
        .find(|entry| matches!(entry, BrowserEntry::Inaccessible(_)))
        .expect("blocked folder must be reported")
    else {
        unreachable!()
    };
    assert_eq!(inaccessible.name, "blocked");
    assert_eq!(
        inaccessible.reason,
        pulseseek_domain::browser::entry::AccessError::PermissionDenied
    );
}

#[test]
fn recursive_stream_continues_after_subdirectory_io_error() {
    use pulseseek_domain::browser::entry::AccessError;

    let dir = tempfile::tempdir().expect("create temp dir");
    let root = dir.path().to_path_buf();
    let gone = root.join("gone");
    let ok_dir = root.join("okdir");
    let ok_file = ok_dir.join("ok.wav");
    create_wav(&ok_file);

    let reader = |target: &Path| -> std::io::Result<Vec<ChildListing>> {
        if target == root {
            Ok(vec![
                ChildListing { path: gone.clone(), is_dir: true, is_symlink: false },
                ChildListing { path: ok_dir.clone(), is_dir: true, is_symlink: false },
            ])
        } else if target == gone {
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "volume disconnected"))
        } else if target == ok_dir {
            Ok(vec![ChildListing { path: ok_file.clone(), is_dir: false, is_symlink: false }])
        } else {
            Ok(vec![])
        }
    };

    let mut entries = Vec::new();
    NativeFolderReader
        .stream_recursive_files_with_reader(
            &root,
            RecursiveReadOptions { show_unsupported: false, show_hidden: false, batch_size: 100 },
            || false,
            |chunk| entries.extend(chunk.iter().cloned()),
            reader,
        )
        .expect("walk must not abort on a vanished subtree");

    assert!(
        entries.iter().any(|entry| entry.name() == "ok.wav"),
        "unaffected subtrees must still be streamed",
    );
    let BrowserEntry::Inaccessible(inaccessible) = entries
        .iter()
        .find(|entry| matches!(entry, BrowserEntry::Inaccessible(_)))
        .expect("vanished subtree must be reported")
    else {
        unreachable!()
    };
    assert_eq!(inaccessible.name, "gone");
    assert_eq!(inaccessible.reason, AccessError::NotFound);
}

#[test]
fn recursive_stream_cancellation_stops_after_first_batch() {
    let dir = tempfile::tempdir().expect("create temp dir");
    for index in 0..10 {
        create_wav(&dir.path().join(format!("song{index}.wav")));
    }
    let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let cancel_flag = cancelled.clone();
    let mut batches = 0;

    NativeFolderReader
        .stream_recursive_files(
            dir.path(),
            false,
            false,
            3,
            || cancel_flag.load(std::sync::atomic::Ordering::Acquire),
            |_chunk| {
                batches += 1;
                cancelled.store(true, std::sync::atomic::Ordering::Release);
            },
        )
        .expect("cancelled walk must return cleanly");

    assert_eq!(batches, 1, "walker must stop as soon as cancellation is requested");
}

#[test]
fn recursive_stream_batches_results() {
    let dir = tempfile::tempdir().expect("create temp dir");
    for index in 0..5 {
        create_wav(&dir.path().join(format!("s{index}.wav")));
    }

    let mut chunk_sizes = Vec::new();
    NativeFolderReader
        .stream_recursive_files(
            dir.path(),
            false,
            false,
            2,
            || false,
            |chunk| {
                chunk_sizes.push(chunk.len());
            },
        )
        .unwrap();

    assert_eq!(chunk_sizes.iter().sum::<usize>(), 5);
    assert!(chunk_sizes.iter().all(|size| *size <= 2));
}

#[test]
fn recursive_stream_respects_show_unsupported() {
    let dir = tempfile::tempdir().expect("create temp dir");
    create_wav(&dir.path().join("a/real.wav"));
    create_file(&dir.path().join("a/notes.txt"));
    create_file(&dir.path().join("a/setup.msi"));

    let hidden = collect_recursive(&NativeFolderReader, dir.path(), false, 100, || false);
    assert_eq!(
        hidden.iter().filter(|entry| matches!(entry, BrowserEntry::PlayableFile(_))).count(),
        1,
        "unsupported files must stay hidden by default in recursive mode",
    );
    assert!(hidden.iter().all(|entry| !matches!(entry, BrowserEntry::UnsupportedFile(_))));

    let revealed = collect_recursive(&NativeFolderReader, dir.path(), true, 100, || false);
    let unsupported: Vec<&str> = revealed
        .iter()
        .filter(|entry| matches!(entry, BrowserEntry::UnsupportedFile(_)))
        .map(|entry| entry.name())
        .collect();
    assert!(unsupported.contains(&"notes.txt"));
    assert!(unsupported.contains(&"setup.msi"));
}

#[test]
fn recursive_stream_empty_root_emits_nothing() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let mut batches = 0;
    NativeFolderReader
        .stream_recursive_files(dir.path(), false, false, 100, || false, |_chunk| batches += 1)
        .unwrap();
    assert_eq!(batches, 0);
}

#[test]
fn recursive_stream_root_missing_returns_error() {
    let result = NativeFolderReader.stream_recursive_files(
        Path::new("/nonexistent/recursive/path"),
        false,
        false,
        100,
        || false,
        |_chunk| {},
    );
    let err = result.expect_err("missing root must fail");
    assert_eq!(err.user_descriptor().category(), pulseseek_domain::error::ErrorCategory::NotFound);
}
