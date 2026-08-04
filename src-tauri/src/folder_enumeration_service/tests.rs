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
        entries.iter().all(|entry| entry.kind != "unsupported"),
        "non-audio files must not be emitted without an explicit reveal request: {:?}",
        entries.iter().map(|entry| entry.name.as_str()).collect::<Vec<_>>(),
    );
    assert_eq!(
        entries
            .iter()
            .filter(|entry| entry.kind == "playable")
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        vec!["real.wav"],
        "only decoder-validated audio files may be marked playable",
    );
    assert!(
        entries.iter().all(|entry| entry.name != "misleading.wav"),
        "a file that merely has an audio extension must not be listed before decoder validation",
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
    });
    let data = browser_entry_to_data(&entry);
    assert_eq!(data.id, "/music/beats");
    assert_eq!(data.name, "beats");
    assert_eq!(data.kind, "folder");
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
