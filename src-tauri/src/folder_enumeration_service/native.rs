use std::sync::atomic::Ordering;
use std::sync::Arc;

use pulseseek_domain::error::{ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory};

use crate::playback_events::{
    BrowserEntryData, FolderChunkPayload, PlayableFileMetadataData, PlaybackEventEmitter,
    EVENT_FOLDER_CHUNK,
};

use super::{ActiveEnumerations, FolderEnumerationService};

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

pub(crate) const MAX_JAVASCRIPT_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

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
