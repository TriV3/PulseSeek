use std::collections::HashSet;
#[cfg(unix)]
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use pulseseek_domain::error::{ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory};

use crate::file_watcher_service::FileWatcherService;
use crate::playback_events::{
    BrowserEntryData, FolderChunkPayload, PlayableFileMetadataData, PlaybackEventEmitter,
    EVENT_FOLDER_CHUNK,
};

use super::{
    ActiveEnumerations, BrowserLibraryData, BrowserLibraryKind, BrowserRootData, BrowserRootKind,
    FolderEnumerationOptions, FolderEnumerationService,
};

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
        options: FolderEnumerationOptions,
        active: &ActiveEnumerations,
        events: Arc<dyn PlaybackEventEmitter>,
    ) -> Result<String, ApplicationError> {
        crate::path_validation::validate_directory(path)?;
        if options.batch_size == 0 {
            return Err(ApplicationError::new(
                ErrorCategory::InvalidInput,
                DiagnosticContext::new(DiagnosticCode::BrowserRead),
                std::io::Error::other("batch size must be greater than zero"),
            ));
        }

        // Watch the browsed folder so external changes refresh the frontend
        // and invalidate stale waveform cache rows (FR-BR-008, FR-FM-010).
        // The watch runs on a dedicated thread so a slow filesystem watcher
        // can never block enumeration or the command loop. The filesystem and
        // Home roots are skipped: recursively watching either includes large
        // unrelated trees and PulseSeek's own writes, which can stall FSEvents
        // or create a refresh feedback loop.
        if let Some(watcher) = self.watcher.clone() {
            let generation = self.watch_generation.fetch_add(1, Ordering::AcqRel) + 1;
            let current_generation = Arc::clone(&self.watch_generation);
            if !should_skip_recursive_watch(path) {
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

                if options.recursive {
                    let result = reader.stream_recursive_files(
                        std::path::Path::new(&path),
                        options.show_unsupported,
                        options.show_hidden,
                        options.batch_size,
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
                    options.show_unsupported,
                    options.show_hidden,
                    options.batch_size,
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

                let result = reader.stream_folder_files_parallel(
                    std::path::Path::new(&path),
                    options.show_unsupported,
                    options.batch_size,
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
                if let Err(error) = result {
                    tracing::warn!(
                        path = %path,
                        error = %error,
                        "folder enumeration failed, sending empty result",
                    );
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

pub(crate) fn should_skip_recursive_watch(path: &str) -> bool {
    if path == std::path::MAIN_SEPARATOR.to_string() {
        return true;
    }
    let home_variable = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(home_variable)
        .map(PathBuf::from)
        .is_some_and(|home| home == std::path::Path::new(path))
}

fn system_roots() -> Vec<BrowserRootData> {
    let mut roots = Vec::new();

    #[cfg(unix)]
    roots.push(BrowserRootData {
        path: "/".to_string(),
        name: "System".to_string(),
        kind: BrowserRootKind::System,
    });

    if let Some(home) = user_home_directory() {
        roots.push(BrowserRootData {
            path: home.to_string_lossy().into_owned(),
            name: "Home".to_string(),
            kind: BrowserRootKind::Home,
        });
    }

    #[cfg(target_os = "macos")]
    add_child_mounts(Path::new("/Volumes"), &network_mount_paths(), &mut roots);

    #[cfg(target_os = "linux")]
    {
        let network_mounts = network_mount_paths();
        add_child_mounts(Path::new("/mnt"), &network_mounts, &mut roots);
        add_child_mounts(Path::new("/media"), &network_mounts, &mut roots);
        add_child_mounts(Path::new("/run/media"), &network_mounts, &mut roots);
    }

    #[cfg(windows)]
    for letter in b'A'..=b'Z' {
        let path = format!("{}:\\", char::from(letter));
        if std::path::Path::new(&path).is_dir() {
            roots.push(BrowserRootData {
                name: path.clone(),
                path,
                kind: BrowserRootKind::Physical,
            });
        }
    }

    roots.sort_by(|left, right| {
        root_rank(left.kind)
            .cmp(&root_rank(right.kind))
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    roots.dedup_by(|left, right| left.path == right.path);
    roots
}

fn user_home_directory() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
}

fn root_rank(kind: BrowserRootKind) -> u8 {
    match kind {
        BrowserRootKind::System => 0,
        BrowserRootKind::Home => 1,
        BrowserRootKind::Physical => 2,
        BrowserRootKind::Network => 3,
    }
}

pub(super) fn system_libraries() -> Vec<BrowserLibraryData> {
    let Some(home) = user_home_directory() else {
        return Vec::new();
    };
    let candidates = [
        ("Documents", "Documents", BrowserLibraryKind::Documents),
        ("Music", "Music", BrowserLibraryKind::Music),
        ("Pictures", "Pictures", BrowserLibraryKind::Pictures),
        (
            "Videos",
            if cfg!(target_os = "macos") { "Movies" } else { "Videos" },
            BrowserLibraryKind::Videos,
        ),
        ("Downloads", "Downloads", BrowserLibraryKind::Downloads),
    ];
    candidates
        .into_iter()
        .filter_map(|(name, child, kind)| {
            let path = home.join(child);
            path.is_dir().then(|| BrowserLibraryData {
                path: path.to_string_lossy().into_owned(),
                name: name.to_string(),
                kind,
            })
        })
        .collect()
}

#[cfg(unix)]
fn add_child_mounts(
    parent: &Path,
    network_mounts: &HashSet<PathBuf>,
    roots: &mut Vec<BrowserRootData>,
) {
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
            kind: if network_mounts.contains(&path) {
                BrowserRootKind::Network
            } else {
                BrowserRootKind::Physical
            },
        });
    }
}

#[cfg(target_os = "macos")]
fn network_mount_paths() -> HashSet<PathBuf> {
    std::process::Command::new("/sbin/mount")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| parse_network_mount_paths(&String::from_utf8_lossy(&output.stdout)))
        .unwrap_or_default()
}

#[cfg(target_os = "linux")]
fn network_mount_paths() -> HashSet<PathBuf> {
    std::fs::read_to_string("/proc/self/mounts")
        .ok()
        .map(|mounts| parse_linux_network_mount_paths(&mounts))
        .unwrap_or_default()
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn network_mount_paths() -> HashSet<PathBuf> {
    HashSet::new()
}

pub(crate) fn parse_network_mount_paths(output: &str) -> HashSet<PathBuf> {
    output
        .lines()
        .filter_map(|line| {
            let (_, mounted) = line.split_once(" on ")?;
            let (path, options) = mounted.rsplit_once(" (")?;
            let file_system = options.trim_end_matches(')').split(',').next()?.trim();
            is_network_file_system(file_system).then(|| PathBuf::from(path))
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn parse_linux_network_mount_paths(output: &str) -> HashSet<PathBuf> {
    output
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let _source = fields.next()?;
            let path = fields.next()?;
            let file_system = fields.next()?;
            is_network_file_system(file_system).then(|| PathBuf::from(path.replace("\\040", " ")))
        })
        .collect()
}

fn is_network_file_system(file_system: &str) -> bool {
    matches!(
        file_system.to_ascii_lowercase().as_str(),
        "smbfs"
            | "cifs"
            | "afpfs"
            | "nfs"
            | "nfs4"
            | "webdav"
            | "webdavfs"
            | "davfs"
            | "davfs2"
            | "sshfs"
            | "fuse.sshfs"
            | "9p"
    )
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
        has_subfolders: match entry {
            pulseseek_domain::browser::entry::BrowserEntry::Folder(folder) => folder.has_subfolders,
            _ => None,
        },
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
