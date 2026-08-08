use std::sync::Arc;
use std::time::{Duration, Instant};

use super::*;
use crate::playback_events::{
    BrowserEntryData, FakeEventEmitter, FolderChunkPayload, EVENT_FOLDER_CHUNK,
};
use pulseseek_domain::error::ErrorContract;

#[test]
fn active_enumerations_register_and_cancel() {
    let active = ActiveEnumerations::new();
    let flag = active.register("session-1");
    assert!(!flag.load(Ordering::Acquire), "new session should not be cancelled");
    active.cancel("session-1");
    assert!(flag.load(Ordering::Acquire), "session should be cancelled");
}

#[test]
fn active_enumerations_remove_unknown_idempotent() {
    let active = ActiveEnumerations::new();
    active.remove("nonexistent");
}

#[test]
fn active_enumerations_cancel_unknown_idempotent() {
    let active = ActiveEnumerations::new();
    active.cancel("nonexistent");
}

#[test]
fn fake_service_starts_enumeration() {
    let mut service = FakeFolderEnumerationService::new();
    let active = ActiveEnumerations::new();
    let events = Arc::new(FakeEventEmitter::new()) as Arc<dyn PlaybackEventEmitter>;

    let session_id =
        service.start_enumeration("/music", 50, false, false, &active, events).unwrap();

    assert_eq!(service.start_call_count, 1);
    assert_eq!(service.last_path, Some("/music".to_string()));
    assert_eq!(service.last_batch_size, Some(50));
    assert_eq!(session_id, "test-session-001");
}

#[test]
fn set_watcher_via_trait_starts_watching_on_enumeration() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use crate::file_watcher_service::FileWatcherService;
    use pulseseek_domain::error::ApplicationError;

    struct CountingWatcher {
        calls: Arc<AtomicUsize>,
    }
    impl FileWatcherService for CountingWatcher {
        fn start_watching(&mut self, _path: &str) -> Result<(), ApplicationError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn stop_watching(&mut self) -> Result<(), ApplicationError> {
            Ok(())
        }
        fn watched_path(&self) -> Option<String> {
            None
        }
    }

    let dir = tempfile::tempdir().expect("temp dir");
    let calls = Arc::new(AtomicUsize::new(0));
    let mut service: Box<dyn FolderEnumerationService> =
        Box::new(NativeFolderEnumerationService::new());
    service.set_watcher(Box::new(CountingWatcher { calls: Arc::clone(&calls) }));

    let active = ActiveEnumerations::new();
    let events =
        Arc::new(crate::playback_events::NoopEventEmitter) as Arc<dyn PlaybackEventEmitter>;
    service
        .start_enumeration(&dir.path().to_string_lossy(), 50, false, false, &active, events)
        .expect("enumeration starts");

    // The watch runs on a dedicated thread; wait for it to complete.
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && calls.load(Ordering::SeqCst) == 0 {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "start_enumeration must start watching through the trait-injected watcher"
    );
}

#[test]
fn watcher_skips_filesystem_root() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use crate::file_watcher_service::FileWatcherService;
    use pulseseek_domain::error::ApplicationError;

    struct CountingWatcher {
        calls: Arc<AtomicUsize>,
    }
    impl FileWatcherService for CountingWatcher {
        fn start_watching(&mut self, _path: &str) -> Result<(), ApplicationError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn stop_watching(&mut self) -> Result<(), ApplicationError> {
            Ok(())
        }
        fn watched_path(&self) -> Option<String> {
            None
        }
    }

    let calls = Arc::new(AtomicUsize::new(0));
    let mut service: Box<dyn FolderEnumerationService> =
        Box::new(NativeFolderEnumerationService::new());
    service.set_watcher(Box::new(CountingWatcher { calls: Arc::clone(&calls) }));

    let active = ActiveEnumerations::new();
    let events =
        Arc::new(crate::playback_events::NoopEventEmitter) as Arc<dyn PlaybackEventEmitter>;
    service
        .start_enumeration("/", 50, false, false, &active, events)
        .expect("root enumeration starts");

    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "the filesystem root must never be watched recursively"
    );
}

#[test]
fn fake_service_fails_with_error() {
    let mut service = FakeFolderEnumerationService::new();
    service.fail_start = true;
    let active = ActiveEnumerations::new();
    let events = Arc::new(FakeEventEmitter::new()) as Arc<dyn PlaybackEventEmitter>;

    let result = service.start_enumeration("/music", 50, false, false, &active, events);

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().user_descriptor().category(), ErrorCategory::Unavailable);
}

#[test]
fn fake_service_records_show_unsupported_preference() {
    let mut service = FakeFolderEnumerationService::new();
    let active = ActiveEnumerations::new();
    let events = Arc::new(FakeEventEmitter::new()) as Arc<dyn PlaybackEventEmitter>;

    service.start_enumeration("/music", 50, true, false, &active, events).unwrap();

    assert_eq!(service.last_show_unsupported, Some(true));
}

#[test]
fn fake_service_records_recursive_flag() {
    let mut service = FakeFolderEnumerationService::new();
    let active = ActiveEnumerations::new();
    let events = Arc::new(FakeEventEmitter::new()) as Arc<dyn PlaybackEventEmitter>;

    service.start_enumeration("/music", 50, false, true, &active, events).unwrap();

    assert_eq!(service.last_recursive, Some(true));
}

#[test]
fn native_service_lists_system_and_mounted_roots() {
    let service = NativeFolderEnumerationService::new();

    let roots = service.list_roots().unwrap();

    assert!(!roots.is_empty());
    assert!(roots.iter().all(|root| std::path::Path::new(&root.path).is_dir()));
    assert!(roots.iter().any(|root| root.path == std::path::MAIN_SEPARATOR.to_string()));
}

#[test]
fn native_service_rejects_a_missing_saved_folder_before_starting_a_worker() {
    let mut service = NativeFolderEnumerationService::new();
    let active = ActiveEnumerations::new();
    let events = Arc::new(FakeEventEmitter::new()) as Arc<dyn PlaybackEventEmitter>;
    let missing = tempfile::tempdir().unwrap().path().join("removed-folder");

    let error = service
        .start_enumeration(&missing.to_string_lossy(), 50, false, false, &active, events)
        .expect_err("missing saved folder must fail synchronously");

    assert_eq!(error.user_descriptor().category(), ErrorCategory::InvalidInput);
}

fn folder_chunk_entries(events: &Arc<FakeEventEmitter>) -> Vec<BrowserEntryData> {
    let mut entries = Vec::new();
    for envelope in events.recorded_events() {
        if envelope.event != EVENT_FOLDER_CHUNK {
            continue;
        }
        let payload: FolderChunkPayload =
            serde_json::from_value(envelope.payload.clone()).expect("folder chunk payload");
        entries.extend(payload.entries);
    }
    entries
}

fn folder_chunk_payloads(events: &Arc<FakeEventEmitter>) -> Vec<FolderChunkPayload> {
    events
        .recorded_events()
        .iter()
        .filter(|envelope| envelope.event == EVENT_FOLDER_CHUNK)
        .map(|envelope| {
            serde_json::from_value(envelope.payload.clone()).expect("folder chunk payload")
        })
        .collect()
}

fn wait_for_enumeration_done(events: &Arc<FakeEventEmitter>) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let done = events.recorded_events().iter().any(|envelope| {
            if envelope.event != EVENT_FOLDER_CHUNK {
                return false;
            }
            let payload: FolderChunkPayload =
                serde_json::from_value(envelope.payload.clone()).expect("folder chunk payload");
            payload.done
        });
        if done {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("folder enumeration did not finish within the timeout");
}

fn create_mixed_folder() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("create temp dir");
    std::fs::write(
        dir.path().join("real.wav"),
        include_bytes!(
            "../../../crates/pulseseek-decoder-symphonia/tests/fixtures/silent-stereo-44100.wav"
        ),
    )
    .expect("write WAV fixture");
    std::fs::write(
        dir.path().join("setup.msi"),
        vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1, 0, 0, 0, 0],
    )
    .expect("write MSI file");
    std::fs::write(dir.path().join("disk.dmg"), b"not-an-audio-file").expect("write DMG file");
    std::fs::write(dir.path().join("notes.txt"), b"notes").expect("write text file");
    std::fs::write(dir.path().join("misleading.wav"), b"not-a-wave-file")
        .expect("write misleading WAV file");
    dir
}

#[test]
fn native_enumeration_omits_non_audio_files_by_default() {
    let dir = create_mixed_folder();
    let events = Arc::new(FakeEventEmitter::new());
    let active = ActiveEnumerations::new();
    let mut service = NativeFolderEnumerationService::new();

    service
        .start_enumeration(
            &dir.path().to_string_lossy(),
            50,
            false,
            false,
            &active,
            events.clone() as Arc<dyn PlaybackEventEmitter>,
        )
        .expect("start enumeration");
    wait_for_enumeration_done(&events);

    let entries = folder_chunk_entries(&events);
    assert!(
        entries.iter().any(|entry| entry.kind == "playable"),
        "audio files should still be emitted",
    );
    assert!(
        entries.iter().all(|entry| {
            !matches!(entry.name.as_str(), "setup.msi" | "disk.dmg" | "notes.txt")
        }),
        "files without a supported audio extension must stay hidden: {:?}",
        entries.iter().map(|entry| entry.name.as_str()).collect::<Vec<_>>()
    );
    assert!(
        entries.iter().any(|entry| {
            entry.name == "real.wav" && entry.kind == "playable" && entry.metadata.is_none()
        }),
        "recognized audio names should be previewed before metadata is ready"
    );
    assert!(
        entries.iter().any(|entry| {
            entry.name == "real.wav" && entry.kind == "playable" && entry.metadata.is_some()
        }),
        "validated audio must replace its preview with metadata"
    );
    assert!(
        entries.iter().any(|entry| entry.name == "misleading.wav" && entry.kind == "unsupported"),
        "a rejected preview candidate must emit a removal tombstone"
    );
}

#[test]
fn native_enumeration_streams_large_folders_in_small_playable_batches() {
    let dir = tempfile::tempdir().expect("create temp folder");
    for index in 0..24 {
        std::fs::write(
            dir.path().join(format!("sample-{index:02}.wav")),
            include_bytes!(
                "../../../crates/pulseseek-decoder-symphonia/tests/fixtures/silent-stereo-44100.wav"
            ),
        )
        .expect("write WAV fixture");
    }
    let events = Arc::new(FakeEventEmitter::new());
    let active = ActiveEnumerations::new();
    let mut service = NativeFolderEnumerationService::new();

    service
        .start_enumeration(
            &dir.path().to_string_lossy(),
            100,
            false,
            false,
            &active,
            events.clone() as Arc<dyn PlaybackEventEmitter>,
        )
        .expect("start enumeration");
    wait_for_enumeration_done(&events);

    let verified_chunks: Vec<_> = folder_chunk_payloads(&events)
        .into_iter()
        .filter(|chunk| {
            chunk.entries.iter().any(|entry| entry.kind == "playable" && entry.metadata.is_some())
        })
        .collect();
    assert!(
        verified_chunks.len() > 1,
        "verified files must be rendered progressively instead of waiting for the whole folder"
    );
    assert!(
        verified_chunks.first().expect("first verified chunk").entries.len() <= 4,
        "the first playable rows must be emitted with minimal latency"
    );
    assert!(
        verified_chunks.iter().all(|chunk| chunk.entries.len() <= 16),
        "interactive enumeration batches must stay small even when the requested batch is large"
    );
}

#[test]
fn native_enumeration_emits_unsupported_files_when_requested() {
    let dir = create_mixed_folder();
    let events = Arc::new(FakeEventEmitter::new());
    let active = ActiveEnumerations::new();
    let mut service = NativeFolderEnumerationService::new();

    service
        .start_enumeration(
            &dir.path().to_string_lossy(),
            50,
            true,
            false,
            &active,
            events.clone() as Arc<dyn PlaybackEventEmitter>,
        )
        .expect("start enumeration");
    wait_for_enumeration_done(&events);

    let entries = folder_chunk_entries(&events);
    assert!(
        entries.iter().any(|entry| entry.kind == "unsupported"),
        "unsupported files should be emitted when explicitly requested",
    );
    assert!(
        entries
            .iter()
            .filter(|entry| matches!(entry.name.as_str(), "setup.msi" | "disk.dmg"))
            .all(|entry| entry.kind == "unsupported"),
        "disk images and installers must never be classified as playable",
    );
}

#[test]
fn native_service_recursive_emits_all_subtree_files() {
    let dir = tempfile::tempdir().expect("create temp dir");
    std::fs::create_dir_all(dir.path().join("sub")).expect("create subfolder");
    std::fs::write(
        dir.path().join("top.wav"),
        include_bytes!(
            "../../../crates/pulseseek-decoder-symphonia/tests/fixtures/silent-stereo-44100.wav"
        ),
    )
    .expect("write top WAV");
    std::fs::write(
        dir.path().join("sub").join("nested.wav"),
        include_bytes!(
            "../../../crates/pulseseek-decoder-symphonia/tests/fixtures/silent-stereo-44100.wav"
        ),
    )
    .expect("write nested WAV");
    let events = Arc::new(FakeEventEmitter::new());
    let active = ActiveEnumerations::new();
    let mut service = NativeFolderEnumerationService::new();

    service
        .start_enumeration(
            &dir.path().to_string_lossy(),
            50,
            false,
            true,
            &active,
            events.clone() as Arc<dyn PlaybackEventEmitter>,
        )
        .expect("start recursive enumeration");
    wait_for_enumeration_done(&events);

    let entries = folder_chunk_entries(&events);
    let playable: Vec<&str> = entries
        .iter()
        .filter(|entry| entry.kind == "playable")
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(
        playable,
        vec!["top.wav", "nested.wav"],
        "recursive mode must stream files from every subtree level",
    );
    assert!(
        entries.iter().all(|entry| entry.kind != "folder"),
        "recursive mode streams files only, never folder rows",
    );
    let chunks = folder_chunk_payloads(&events);
    assert!(
        chunks.iter().filter(|chunk| !chunk.done).all(|chunk| !chunk.folders_done),
        "loading state must stay active until the recursive walk completes",
    );
    assert!(
        chunks.last().expect("done chunk").done,
        "recursive enumeration must end with a done chunk",
    );
}

#[test]
fn browser_entry_to_data_converts_folder() {
    use pulseseek_domain::browser::entry::{EntryId, FolderEntry};
    let entry = pulseseek_domain::browser::entry::BrowserEntry::Folder(FolderEntry {
        id: EntryId::new("/music/beats"),
        name: "beats".to_string(),
        has_subfolders: Some(false),
    });
    let data = browser_entry_to_data(&entry);
    assert_eq!(data.id, "/music/beats");
    assert_eq!(data.name, "beats");
    assert_eq!(data.kind, "folder");
    assert_eq!(data.has_subfolders, Some(false));
}

#[test]
fn browser_entry_to_data_converts_playable() {
    use pulseseek_domain::browser::entry::{EntryId, PlayableFileEntry};
    let entry = pulseseek_domain::browser::entry::BrowserEntry::PlayableFile(PlayableFileEntry {
        id: EntryId::new("/music/kick.wav"),
        name: "kick.wav".to_string(),
        metadata: None,
    });
    let data = browser_entry_to_data(&entry);
    assert_eq!(data.kind, "playable");
}

#[test]
fn emit_folder_chunk_emits_correct_event() {
    use pulseseek_domain::browser::entry::{EntryId, PlayableFileEntry};
    let entry = pulseseek_domain::browser::entry::BrowserEntry::PlayableFile(PlayableFileEntry {
        id: EntryId::new("/a.wav"),
        name: "a.wav".to_string(),
        metadata: None,
    });
    let events = Arc::new(FakeEventEmitter::new());

    emit_folder_chunk_phase(&*events, "sid-1", &[entry], false, false);

    let recorded = events.recorded_events();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].event, EVENT_FOLDER_CHUNK);
    let payload: FolderChunkPayload = serde_json::from_value(recorded[0].payload.clone()).unwrap();
    assert_eq!(payload.session_id, "sid-1");
    assert_eq!(payload.entries.len(), 1);
    assert!(!payload.done);
}

#[test]
fn emit_folder_chunk_done_flag() {
    let events = Arc::new(FakeEventEmitter::new());
    emit_folder_chunk_phase(&*events, "sid-1", &[], true, true);
    let recorded = events.recorded_events();
    let payload: FolderChunkPayload = serde_json::from_value(recorded[0].payload.clone()).unwrap();
    assert!(payload.done);
}

#[test]
fn browser_entry_to_data_serializes_partial_metadata() {
    use pulseseek_domain::browser::entry::{EntryId, PlayableFileEntry, PlayableFileMetadata};
    let entry = pulseseek_domain::browser::entry::BrowserEntry::PlayableFile(PlayableFileEntry {
        id: EntryId::new("/music/song.mp3"),
        name: "song.mp3".to_string(),
        metadata: Some(PlayableFileMetadata {
            duration_ms: Some(61_000),
            size_bytes: Some(1_572_864),
            modified_at_ms: None,
            channels: Some(2),
            sample_rate: Some(44_100),
            bit_depth: None,
            codec: Some("MP3".to_string()),
        }),
    });

    let data = browser_entry_to_data(&entry);

    let metadata = data.metadata.expect("metadata should be serialized");
    assert_eq!(metadata.duration_ms, Some(61_000));
    assert_eq!(metadata.modified_at_ms, None);
    assert_eq!(metadata.codec.as_deref(), Some("MP3"));
}

#[test]
fn browser_entry_to_data_omits_javascript_unsafe_integers() {
    use pulseseek_domain::browser::entry::{EntryId, PlayableFileEntry, PlayableFileMetadata};
    let entry = pulseseek_domain::browser::entry::BrowserEntry::PlayableFile(PlayableFileEntry {
        id: EntryId::new("/music/huge.wav"),
        name: "huge.wav".to_string(),
        metadata: Some(PlayableFileMetadata {
            duration_ms: None,
            size_bytes: Some(MAX_JAVASCRIPT_SAFE_INTEGER + 1),
            modified_at_ms: None,
            channels: None,
            sample_rate: None,
            bit_depth: None,
            codec: None,
        }),
    });

    let data = browser_entry_to_data(&entry);

    assert_eq!(data.metadata.expect("metadata").size_bytes, None);
}
