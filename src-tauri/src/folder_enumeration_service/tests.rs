use std::sync::Arc;

use super::*;
use crate::playback_events::{FakeEventEmitter, FolderChunkPayload, EVENT_FOLDER_CHUNK};
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

    let session_id = service.start_enumeration("/music", 50, false, &active, events).unwrap();

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

    let result = service.start_enumeration("/music", 50, false, &active, events);

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().user_descriptor().category(), ErrorCategory::Unavailable);
}

#[test]
fn fake_service_records_show_unsupported_preference() {
    let mut service = FakeFolderEnumerationService::new();
    let active = ActiveEnumerations::new();
    let events = Arc::new(FakeEventEmitter::new()) as Arc<dyn PlaybackEventEmitter>;

    service.start_enumeration("/music", 50, true, &active, events).unwrap();

    assert_eq!(service.last_show_unsupported, Some(true));
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
        .start_enumeration(&missing.to_string_lossy(), 50, false, &active, events)
        .expect_err("missing saved folder must fail synchronously");

    assert_eq!(error.user_descriptor().category(), ErrorCategory::InvalidInput);
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
