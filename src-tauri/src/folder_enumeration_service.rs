use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tracing;

use pulseseek_domain::error::{ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory};

use crate::playback_events::{
    BrowserEntryData, FolderChunkPayload, PlayableFileMetadataData, PlaybackEventEmitter,
    EVENT_FOLDER_CHUNK,
};

/// Manages active folder enumeration sessions and their cancellation flags.
#[derive(Clone)]
pub struct ActiveEnumerations {
    pub sessions: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl ActiveEnumerations {
    pub fn new() -> Self {
        Self { sessions: Arc::new(Mutex::new(HashMap::new())) }
    }

    /// Registers a new session with a cancellation flag.
    pub fn register(&self, session_id: &str) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        self.sessions
            .lock()
            .expect("sessions mutex poisoned")
            .insert(session_id.to_string(), flag.clone());
        flag
    }

    /// Sets the cancellation flag for a session. No-op if session unknown.
    pub fn cancel(&self, session_id: &str) {
        if let Some(flag) = self.sessions.lock().expect("sessions mutex poisoned").get(session_id) {
            flag.store(true, Ordering::Release);
        }
    }

    /// Removes a session after enumeration completes.
    pub fn remove(&self, session_id: &str) {
        self.sessions.lock().expect("sessions mutex poisoned").remove(session_id);
    }
}

impl Default for ActiveEnumerations {
    fn default() -> Self {
        Self::new()
    }
}

/// Application service for starting and cancelling folder enumeration.
///
/// The real implementation spawns a background thread that reads the folder
/// and emits chunked events. The fake implementation records calls for
/// test assertions.
pub trait FolderEnumerationService: Send {
    /// Starts enumerating a folder.
    ///
    /// Returns a session_id that can be used to cancel the enumeration.
    fn start_enumeration(
        &mut self,
        path: &str,
        batch_size: usize,
        show_unsupported: bool,
        active: &ActiveEnumerations,
        events: Arc<dyn PlaybackEventEmitter>,
    ) -> Result<String, ApplicationError>;
}

/// Fake implementation of [`FolderEnumerationService`] for tests.
pub struct FakeFolderEnumerationService {
    pub start_call_count: u64,
    pub last_path: Option<String>,
    pub last_batch_size: Option<usize>,
    pub last_show_unsupported: Option<bool>,
    pub fail_start: bool,
    pub next_session_id: String,
}

impl FakeFolderEnumerationService {
    pub fn new() -> Self {
        Self {
            start_call_count: 0,
            last_path: None,
            last_batch_size: None,
            last_show_unsupported: None,
            fail_start: false,
            next_session_id: "test-session-001".to_string(),
        }
    }
}

impl Default for FakeFolderEnumerationService {
    fn default() -> Self {
        Self::new()
    }
}

impl FolderEnumerationService for FakeFolderEnumerationService {
    fn start_enumeration(
        &mut self,
        path: &str,
        batch_size: usize,
        show_unsupported: bool,
        _active: &ActiveEnumerations,
        _events: Arc<dyn PlaybackEventEmitter>,
    ) -> Result<String, ApplicationError> {
        self.start_call_count += 1;
        self.last_path = Some(path.to_string());
        self.last_batch_size = Some(batch_size);
        self.last_show_unsupported = Some(show_unsupported);
        if self.fail_start {
            return Err(ApplicationError::new(
                ErrorCategory::Unavailable,
                DiagnosticContext::new(DiagnosticCode::BrowserRead),
                std::io::Error::other("fake enumeration error"),
            ));
        }
        Ok(self.next_session_id.clone())
    }
}

/// Native folder enumeration service. Reads and probes files on worker thread.
pub struct NativeFolderEnumerationService {
    next_session_id: std::sync::atomic::AtomicU64,
}

impl NativeFolderEnumerationService {
    pub fn new() -> Self {
        Self { next_session_id: std::sync::atomic::AtomicU64::new(1) }
    }
}

impl Default for NativeFolderEnumerationService {
    fn default() -> Self {
        Self::new()
    }
}

impl FolderEnumerationService for NativeFolderEnumerationService {
    fn start_enumeration(
        &mut self,
        path: &str,
        batch_size: usize,
        show_unsupported: bool,
        active: &ActiveEnumerations,
        events: Arc<dyn PlaybackEventEmitter>,
    ) -> Result<String, ApplicationError> {
        if batch_size == 0 {
            return Err(ApplicationError::new(
                ErrorCategory::InvalidInput,
                DiagnosticContext::new(DiagnosticCode::BrowserRead),
                std::io::Error::other("batch size must be greater than zero"),
            ));
        }

        let session_id = format!("folder-{}", self.next_session_id.fetch_add(1, Ordering::Relaxed));
        let cancelled = active.register(&session_id);
        let active_for_thread = active.clone();
        let path = path.to_owned();
        let session_for_thread = session_id.clone();

        std::thread::Builder::new()
            .name("pulseseek-folder-enumeration".to_string())
            .spawn(move || {
                let result = pulseseek_browser_fs::NativeFolderReader
                    .read_folder_with_options(std::path::Path::new(&path), show_unsupported);
                match result {
                    Ok(entries) => {
                        for chunk in entries.chunks(batch_size) {
                            if cancelled.load(Ordering::Acquire) || events.is_disconnected() {
                                break;
                            }
                            emit_folder_chunk(&*events, &session_for_thread, chunk, false);
                        }
                    },
                    Err(error) => {
                        tracing::warn!(
                            path = %path,
                            error = %error,
                            "folder enumeration failed, sending empty result",
                        );
                    },
                }
                // Always emit done=true so the frontend exits its loading
                // state, even when enumeration failed or was cancelled.
                if !cancelled.load(Ordering::Acquire) && !events.is_disconnected() {
                    emit_folder_chunk(&*events, &session_for_thread, &[], true);
                }
                active_for_thread.remove(&session_for_thread);
            })
            .map_err(|error| {
                active.remove(&session_id);
                ApplicationError::new(
                    ErrorCategory::Unavailable,
                    DiagnosticContext::new(DiagnosticCode::BrowserRead),
                    error,
                )
            })?;

        Ok(session_id)
    }
}

/// Converts a domain `BrowserEntry` to a serializable `BrowserEntryData`.
pub fn browser_entry_to_data(
    entry: &pulseseek_domain::browser::entry::BrowserEntry,
) -> BrowserEntryData {
    let kind = match entry {
        pulseseek_domain::browser::entry::BrowserEntry::Folder(_) => "folder",
        pulseseek_domain::browser::entry::BrowserEntry::PlayableFile(_) => "playable",
        pulseseek_domain::browser::entry::BrowserEntry::UnsupportedFile(_) => "unsupported",
        pulseseek_domain::browser::entry::BrowserEntry::Inaccessible(_) => "inaccessible",
    };
    BrowserEntryData {
        id: entry.id().as_str().to_string(),
        name: entry.name().to_string(),
        kind: kind.to_string(),
        metadata: match entry {
            pulseseek_domain::browser::entry::BrowserEntry::PlayableFile(file) => {
                file.metadata.as_ref().map(|metadata| PlayableFileMetadataData {
                    duration_ms: metadata.duration_ms.and_then(js_safe_integer),
                    size_bytes: metadata.size_bytes.and_then(js_safe_integer),
                    modified_at_ms: metadata.modified_at_ms.and_then(js_safe_integer),
                    channels: metadata.channels,
                    sample_rate: metadata.sample_rate,
                    bit_depth: metadata.bit_depth,
                    codec: metadata.codec.clone(),
                })
            },
            _ => None,
        },
    }
}

const MAX_JAVASCRIPT_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

fn js_safe_integer(value: u64) -> Option<u64> {
    (value <= MAX_JAVASCRIPT_SAFE_INTEGER).then_some(value)
}

/// Emits a folder chunk event for a batch of entries.
pub fn emit_folder_chunk(
    events: &dyn PlaybackEventEmitter,
    session_id: &str,
    entries: &[pulseseek_domain::browser::entry::BrowserEntry],
    done: bool,
) {
    let data: Vec<BrowserEntryData> = entries.iter().map(browser_entry_to_data).collect();
    let payload = FolderChunkPayload { session_id: session_id.to_string(), entries: data, done };
    let _ = events.emit(
        EVENT_FOLDER_CHUNK,
        serde_json::to_value(payload).expect("folder chunk serialization"),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playback_events::FakeEventEmitter;
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
        active.remove("nonexistent"); // should not panic
    }

    #[test]
    fn active_enumerations_cancel_unknown_idempotent() {
        let active = ActiveEnumerations::new();
        active.cancel("nonexistent"); // should not panic
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
        let entry =
            pulseseek_domain::browser::entry::BrowserEntry::PlayableFile(PlayableFileEntry {
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
        let entry =
            pulseseek_domain::browser::entry::BrowserEntry::PlayableFile(PlayableFileEntry {
                id: EntryId::new("/a.wav"),
                name: "a.wav".to_string(),
                metadata: None,
            });
        let events = Arc::new(FakeEventEmitter::new());

        emit_folder_chunk(&*events, "sid-1", &[entry], false);

        let recorded = events.recorded_events();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].event, EVENT_FOLDER_CHUNK);
        let payload: FolderChunkPayload =
            serde_json::from_value(recorded[0].payload.clone()).unwrap();
        assert_eq!(payload.session_id, "sid-1");
        assert_eq!(payload.entries.len(), 1);
        assert!(!payload.done);
    }

    #[test]
    fn emit_folder_chunk_done_flag() {
        let events = Arc::new(FakeEventEmitter::new());
        emit_folder_chunk(&*events, "sid-1", &[], true);
        let recorded = events.recorded_events();
        let payload: FolderChunkPayload =
            serde_json::from_value(recorded[0].payload.clone()).unwrap();
        assert!(payload.done);
    }

    #[test]
    fn browser_entry_to_data_serializes_partial_metadata() {
        use pulseseek_domain::browser::entry::{EntryId, PlayableFileEntry, PlayableFileMetadata};
        let entry =
            pulseseek_domain::browser::entry::BrowserEntry::PlayableFile(PlayableFileEntry {
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
        let entry =
            pulseseek_domain::browser::entry::BrowserEntry::PlayableFile(PlayableFileEntry {
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
}
