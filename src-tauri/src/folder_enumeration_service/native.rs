use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use pulseseek_domain::error::{ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory};

use crate::file_watcher_service::FileWatcherService;
use crate::playback_events::{
    BrowserEntryData, FolderChunkPayload, PlayableFileMetadataData, PlaybackEventEmitter,
    EVENT_FOLDER_CHUNK,
};

use super::{ActiveEnumerations, BrowserRootData, FolderEnumerationService};

/// Native folder enumeration service. Reads and probes files on worker thread.
/// When a file watcher is configured, enumeration automatically starts watching
/// the browsed folder so external changes refresh the frontend and invalidate
/// stale waveform cache rows (FR-BR-008, FR-FM-010). The watcher is shared
/// behind an `Arc` so starting a watch never blocks the command loop.
pub struct NativeFolderEnumerationService {
    next_session_id: AtomicU64,
    watcher: Option<Arc<Mutex<Box<dyn FileWatcherService>>>>,
    watch_generation: Arc<AtomicU64>,
}

impl NativeFolderEnumerationService {
    pub fn new() -> Self {
        Self {
            next_session_id: AtomicU64::new(1),
            watcher: None,
            watch_generation: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl Default for NativeFolderEnumerationService {
    fn default() -> Self {
        Self::new()
    }
}

impl FolderEnumerationService for NativeFolderEnumerationService {
    fn list_roots(&self) -> Result<Vec<BrowserRootData>, ApplicationError> {
        Ok(system_roots())
    }

    /// Attaches a file watcher. Called once during Tauri setup through the
    /// trait object so the concrete service receives it via the vtable.
    fn set_watcher(&mut self, watcher: Box<dyn FileWatcherService>) {
        self.watcher = Some(Arc::new(Mutex::new(watcher)));
    }

    fn start_enumeration(
        &mut self,
        path: &str,
        batch_size: usize,
        show_unsupported: bool,
        recursive: bool,
        active: &ActiveEnumerations,
        events: Arc<dyn PlaybackEventEmitter>,
    ) -> Result<String, ApplicationError> {
        crate::path_validation::validate_directory(path)?;
        if batch_size == 0 {
            return Err(ApplicationError::new(
                ErrorCategory::InvalidInput,
                DiagnosticContext::new(DiagnosticCode::BrowserRead),
                std::io::Error::other("batch size must be greater than zero"),
            ));
        }

        // Watch the browsed folder so external changes refresh the frontend
        // and invalidate stale waveform cache rows (FR-BR-008, FR-FM-010).
        // The watch runs on a dedicated thread so a slow filesystem watcher
        // can never block enumeration or the command loop. The filesystem
        // root is skipped: watching "/" recursively triggers a system-wide
        // FSEvents scan that can stall startup on macOS.
        if let Some(watcher) = self.watcher.clone() {
            let generation = self.watch_generation.fetch_add(1, Ordering::AcqRel) + 1;
            let current_generation = Arc::clone(&self.watch_generation);
            if path != "/" {
                let watch_path = path.to_owned();
                let spawned = std::thread::Builder::new()
                    .name("pulseseek-file-watch".to_string())
                    .spawn(move || {
                        // Navigation can queue several enumeration requests.
                        // Only latest request may replace active OS watch;
                        // stale threads must not navigate watcher back.
                        if current_generation.load(Ordering::Acquire) != generation {
                            return;
                        }
                        if let Ok(mut watcher) = watcher.lock() {
                            if current_generation.load(Ordering::Acquire) != generation {
                                return;
                            }
                            if let Err(error) = watcher.start_watching(&watch_path) {
                                tracing::warn!(
                                    error = %error,
                                    path = %watch_path,
                                    "file watcher unavailable; continuing without watching"
                                );
                            }
                        }
                    });
                if let Err(error) = spawned {
                    tracing::warn!(error = %error, "failed to spawn file watcher thread");
                }
            } else {
                let spawned = std::thread::Builder::new()
                    .name("pulseseek-file-watch-stop".to_string())
                    .spawn(move || {
                        if current_generation.load(Ordering::Acquire) == generation {
                            if let Ok(mut watcher) = watcher.lock() {
                                let _ = watcher.stop_watching();
                            }
                        }
                    });
                if let Err(error) = spawned {
                    tracing::warn!(error = %error, "failed to stop file watcher thread");
                }
            }
        }

        let session_id = format!("folder-{}", self.next_session_id.fetch_add(1, Ordering::Relaxed));
        let cancelled = active.register(&session_id);
        let active_for_thread = active.clone();
        let path = path.to_owned();
        let session_for_thread = session_id.clone();

        std::thread::Builder::new()
            .name("pulseseek-folder-enumeration".to_string())
            .spawn(move || {
                let reader = pulseseek_browser_fs::NativeFolderReader;

                if recursive {
                    let result = reader.stream_recursive_files(
                        std::path::Path::new(&path),
                        show_unsupported,
                        batch_size,
                        || cancelled.load(Ordering::Acquire) || events.is_disconnected(),
                        |chunk| {
                            if !cancelled.load(Ordering::Acquire) && !events.is_disconnected() {
                                emit_folder_chunk_phase(
                                    &*events,
                                    &session_for_thread,
                                    chunk,
                                    false,
                                    false,
                                );
                            }
                        },
                    );
                    if let Err(error) = result {
                        tracing::warn!(
                            path = %path,
                            error = %error,
                            "recursive folder enumeration failed, sending empty result",
                        );
                    }
                    // Always emit done=true so the frontend exits its loading
                    // state, even when the walk failed or was cancelled.
                    if !cancelled.load(Ordering::Acquire) && !events.is_disconnected() {
                        emit_folder_chunk_phase(&*events, &session_for_thread, &[], true, true);
                    }
                    active_for_thread.remove(&session_for_thread);
                    return;
                }

                let preview = reader.stream_folder_preview(
                    std::path::Path::new(&path),
                    show_unsupported,
                    batch_size,
                    || cancelled.load(Ordering::Acquire) || events.is_disconnected(),
                    |chunk| {
                        if !cancelled.load(Ordering::Acquire) && !events.is_disconnected() {
                            emit_folder_chunk_phase(
                                &*events,
                                &session_for_thread,
                                chunk,
                                true,
                                false,
                            );
                        }
                    },
                );
                if let Err(error) = preview {
                    tracing::warn!(
                        path = %path,
                        error = %error,
                        "folder preview failed, continuing with verification",
                    );
                }

                let result = reader.read_folder_with_options_cancellable(
                    std::path::Path::new(&path),
                    show_unsupported,
                    || cancelled.load(Ordering::Acquire) || events.is_disconnected(),
                );
                match result {
                    Ok(entries) => {
                        for chunk in entries.chunks(batch_size) {
                            if cancelled.load(Ordering::Acquire) || events.is_disconnected() {
                                break;
                            }
                            emit_folder_chunk_phase(
                                &*events,
                                &session_for_thread,
                                chunk,
                                true,
                                false,
                            );
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
                    emit_folder_chunk_phase(&*events, &session_for_thread, &[], true, true);
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

fn system_roots() -> Vec<BrowserRootData> {
    let mut roots = Vec::new();

    #[cfg(unix)]
    roots.push(BrowserRootData { path: "/".to_string(), name: "System".to_string() });

    #[cfg(target_os = "macos")]
    add_child_mounts(std::path::Path::new("/Volumes"), &mut roots);

    #[cfg(target_os = "linux")]
    {
        add_child_mounts(std::path::Path::new("/mnt"), &mut roots);
        add_child_mounts(std::path::Path::new("/media"), &mut roots);
        add_child_mounts(std::path::Path::new("/run/media"), &mut roots);
    }

    #[cfg(windows)]
    for letter in b'A'..=b'Z' {
        let path = format!("{}:\\", char::from(letter));
        if std::path::Path::new(&path).is_dir() {
            roots.push(BrowserRootData { name: path.clone(), path });
        }
    }

    roots.sort_by(|left, right| {
        (left.path != std::path::MAIN_SEPARATOR.to_string())
            .cmp(&(right.path != std::path::MAIN_SEPARATOR.to_string()))
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    roots.dedup_by(|left, right| left.path == right.path);
    roots
}

#[cfg(unix)]
fn add_child_mounts(parent: &std::path::Path, roots: &mut Vec<BrowserRootData>) {
    let Ok(entries) = std::fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path.canonicalize().is_ok_and(|canonical| canonical == std::path::Path::new("/")) {
            continue;
        }
        roots.push(BrowserRootData {
            name: entry.file_name().to_string_lossy().to_string(),
            path: path.to_string_lossy().to_string(),
        });
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

/// Emits a folder chunk event for either the preview or verification phase.
pub(crate) fn emit_folder_chunk_phase(
    events: &dyn PlaybackEventEmitter,
    session_id: &str,
    entries: &[pulseseek_domain::browser::entry::BrowserEntry],
    folders_done: bool,
    done: bool,
) {
    let data: Vec<BrowserEntryData> = entries.iter().map(browser_entry_to_data).collect();
    let payload = FolderChunkPayload {
        session_id: session_id.to_string(),
        entries: data,
        folders_done,
        done,
    };
    let _ = events.emit(
        EVENT_FOLDER_CHUNK,
        serde_json::to_value(payload).expect("folder chunk serialization"),
    );
}
