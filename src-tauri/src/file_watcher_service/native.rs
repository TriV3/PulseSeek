use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[cfg(test)]
use notify_debouncer_full::notify::PollWatcher as PlatformWatcher;
#[cfg(not(test))]
use notify_debouncer_full::notify::RecommendedWatcher as PlatformWatcher;
use notify_debouncer_full::notify::{Config, RecursiveMode};
use notify_debouncer_full::{new_debouncer_opt, DebounceEventResult, Debouncer, NoCache};
use pulseseek_cache::waveform_cache::{waveform_cache_key, WaveformCachePort, WaveformIdentity};
use pulseseek_domain::error::{ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory};

use crate::playback_events::{FileChangePayload, PlaybackEventEmitter, EVENT_FILE_CHANGE};

use super::FileWatcherService;

/// Default debounce window used to coalesce filesystem events.
pub const DEFAULT_DEBOUNCE_MS: u64 = 200;

#[cfg(test)]
const TEST_POLL_INTERVAL_MS: u64 = 20;

/// Native file watcher backed by `notify-debouncer-full`.
///
/// The debouncer runs on its own thread and coalesces bursts. On a debounced
/// batch the service invalidates cached waveforms for modified or removed
/// direct children and emits one `browser:file-change` event carrying the
/// watched folder path. Backends such as macOS FSEvents require a recursive
/// watch to report direct-child changes reliably, so nested events are filtered
/// before logging or emitting anything. Broad roots such as `/` and `Home` are
/// never watched because they include PulseSeek's own caches and logs and could
/// feed internal writes back into an endless refresh cycle.
///
/// `NoCache` is used instead of the recommended file-ID cache: the
/// recommended cache walks the whole watched tree on every `watch()` call,
/// which stalls on large folders such as a home directory. This service only
/// needs a coarse "something changed" signal, so rename stitching accuracy is
/// not worth blocking enumeration.
pub struct NativeFileWatcherService {
    debouncer: Mutex<Debouncer<PlatformWatcher, NoCache>>,
    watched: Arc<Mutex<Option<PathBuf>>>,
}

impl NativeFileWatcherService {
    /// Creates a watcher with the given debounce window in milliseconds.
    ///
    /// `cache` may be `None` when the technical cache failed to open; the
    /// watcher then still emits refresh events but cannot invalidate rows.
    pub fn new(
        events: Arc<dyn PlaybackEventEmitter>,
        cache: Option<Arc<dyn WaveformCachePort>>,
        debounce_ms: u64,
    ) -> Result<Self, ApplicationError> {
        let watched = Arc::new(Mutex::new(None::<PathBuf>));
        let callback_watched = Arc::clone(&watched);
        let callback_events = Arc::clone(&events);
        let callback_cache = cache.clone();
        let debouncer = new_debouncer_opt::<_, PlatformWatcher, NoCache>(
            Duration::from_millis(debounce_ms),
            None,
            move |result: DebounceEventResult| {
                on_debounced(
                    result,
                    &callback_watched,
                    &*callback_events,
                    callback_cache.as_deref(),
                );
            },
            NoCache,
            watcher_config(),
        )
        .map_err(|error| {
            ApplicationError::new(
                ErrorCategory::Unavailable,
                DiagnosticContext::new(DiagnosticCode::BrowserRead),
                error,
            )
        })?;

        Ok(Self { debouncer: Mutex::new(debouncer), watched })
    }

    /// Creates a watcher with the default debounce window.
    pub fn with_defaults(
        events: Arc<dyn PlaybackEventEmitter>,
        cache: Option<Arc<dyn WaveformCachePort>>,
    ) -> Result<Self, ApplicationError> {
        Self::new(events, cache, DEFAULT_DEBOUNCE_MS)
    }
}

fn watcher_config() -> Config {
    #[cfg(test)]
    {
        // Sandboxed macOS test runners may not deliver FSEvents at all. The
        // official polling backend still exercises real filesystem changes,
        // debounce, invalidation, and lifecycle deterministically.
        Config::default()
            .with_poll_interval(Duration::from_millis(TEST_POLL_INTERVAL_MS))
            .with_compare_contents(true)
    }
    #[cfg(not(test))]
    {
        Config::default()
    }
}

impl FileWatcherService for NativeFileWatcherService {
    fn start_watching(&mut self, path: &str) -> Result<(), ApplicationError> {
        tracing::debug!(path = %path, "file watcher start_watching begin");
        let target = PathBuf::from(path);
        if !target.is_dir() {
            return Err(ApplicationError::new(
                ErrorCategory::InvalidInput,
                DiagnosticContext::new(DiagnosticCode::BrowserRead),
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "watch target is not an existing directory",
                ),
            ));
        }
        let mut debouncer = self.debouncer.lock().expect("debouncer mutex poisoned");
        let mut watched = self.watched.lock().expect("watched mutex poisoned");
        if watched.as_ref() == Some(&target) {
            // Enumeration refreshes call start_watching again. Re-watching
            // the same folder would tear down and recreate the OS watch,
            // producing another filesystem event and an endless UI refresh
            // loop.
            return Ok(());
        }
        if let Some(current) = watched.take() {
            // Single active watch: replacing the previous folder is not an
            // error worth surfacing when the underlying watch already lapsed.
            let _ = debouncer.unwatch(&current);
        }
        debouncer.watch(&target, RecursiveMode::Recursive).map_err(watcher_error)?;
        *watched = Some(target);
        tracing::debug!(path = %path, "file watcher start_watching end");
        Ok(())
    }

    fn stop_watching(&mut self) -> Result<(), ApplicationError> {
        let mut debouncer = self.debouncer.lock().expect("debouncer mutex poisoned");
        let mut watched = self.watched.lock().expect("watched mutex poisoned");
        if let Some(current) = watched.take() {
            let _ = debouncer.unwatch(&current);
        }
        Ok(())
    }

    fn watched_path(&self) -> Option<String> {
        self.watched
            .lock()
            .expect("watched mutex poisoned")
            .as_ref()
            .map(|path| path.to_string_lossy().to_string())
    }
}

/// Maps a notify error to the application error contract.
fn watcher_error(error: notify_debouncer_full::notify::Error) -> ApplicationError {
    use notify_debouncer_full::notify::ErrorKind;
    let category = match error.kind {
        ErrorKind::PathNotFound => ErrorCategory::InvalidInput,
        _ => ErrorCategory::Unavailable,
    };
    ApplicationError::new(category, DiagnosticContext::new(DiagnosticCode::BrowserRead), error)
}

/// Handles one debounced batch from the watcher thread.
///
/// Runs off the UI and audio threads. Invalidates cached waveforms for
/// changed sources, then emits a single refresh event for the currently
/// watched folder.
pub(crate) fn on_debounced(
    result: DebounceEventResult,
    watched: &Mutex<Option<PathBuf>>,
    events: &dyn PlaybackEventEmitter,
    cache: Option<&dyn WaveformCachePort>,
) {
    match result {
        Ok(debounced) => {
            let Some(path) = watched.lock().expect("watched mutex poisoned").clone() else {
                return;
            };
            let changed = debounced
                .iter()
                .filter(|event| {
                    is_content_change(&event.event)
                        && event_affects_watched_folder(&event.event, &path)
                })
                .collect::<Vec<_>>();
            if changed.is_empty() {
                return;
            }
            tracing::debug!(count = changed.len(), "file watcher debounced content changes");
            for event in changed {
                invalidate_changed(&event.event, cache);
            }
            let payload = FileChangePayload { path: path.to_string_lossy().to_string() };
            let emit_result = events.emit(
                EVENT_FILE_CHANGE,
                serde_json::to_value(payload).expect("file change payload"),
            );
            tracing::debug!(result = ?emit_result, path = %path.display(), "file watcher emitted event");
        },
        Err(errors) => {
            for error in &errors {
                tracing::warn!(error = ?error, "file watcher reported an error");
            }
        },
    }
}

/// Rejects events queued by a previous watch and changes below nested
/// directories. Neither can affect the direct children currently rendered by
/// the browser.
fn event_affects_watched_folder(
    event: &notify_debouncer_full::notify::Event,
    watched: &std::path::Path,
) -> bool {
    let watched = normalized_path(watched);
    event.paths.iter().any(|path| {
        normalized_path(path) == watched
            || path.parent().is_some_and(|parent| normalized_path(parent) == watched)
    })
}

/// Resolves platform aliases for existing paths (notably `/var` versus
/// `/private/var` on macOS). For removed files, resolving the surviving parent
/// still yields a stable comparable path.
fn normalized_path(path: &std::path::Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| {
        path.parent()
            .and_then(|parent| parent.canonicalize().ok())
            .and_then(|parent| path.file_name().map(|name| parent.join(name)))
            .unwrap_or_else(|| path.to_path_buf())
    })
}

/// True when an event can change the visible directory contents or audio
/// bytes. Reads/accesses and metadata-only changes are deliberately ignored:
/// enumeration itself can generate them on some local and network filesystems,
/// and feeding them back into enumeration creates a refresh loop.
fn is_content_change(event: &notify_debouncer_full::notify::Event) -> bool {
    use notify_debouncer_full::notify::event::ModifyKind;
    use notify_debouncer_full::notify::EventKind;

    match event.kind {
        EventKind::Create(_) | EventKind::Remove(_) | EventKind::Any => true,
        EventKind::Modify(ModifyKind::Metadata(_)) => false,
        EventKind::Modify(_) => true,
        EventKind::Access(_) | EventKind::Other => false,
    }
}

/// Deletes the cached waveform for a changed source.
///
/// Creation, modification, and removal can all leave a stale row behind: some
/// platforms report an overwrite as a create event, and a removed file is never
/// revisited by the self-healing load path. Access and meta events do not.
fn invalidate_changed(
    event: &notify_debouncer_full::notify::Event,
    cache: Option<&dyn WaveformCachePort>,
) {
    if !event.kind.is_create() && !event.kind.is_modify() && !event.kind.is_remove() {
        return;
    }
    let Some(cache) = cache else { return };
    for path in &event.paths {
        for candidate in cache_key_candidates(path) {
            let key = waveform_cache_key(&WaveformIdentity::new(candidate, 0, 0));
            if let Err(error) = cache.delete_waveform(&key) {
                tracing::warn!(
                    error = %error,
                    path = %path.display(),
                    "waveform cache invalidation failed"
                );
            }
        }
    }
}

/// Path forms that may have produced the stored cache key.
///
/// The key scheme canonicalizes paths, so a removed file is matched through its
/// canonical parent directory even when the delivered event path is gone.
fn cache_key_candidates(path: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut candidates = vec![path.to_path_buf()];
    if let Ok(canonical) = std::fs::canonicalize(path) {
        candidates.push(canonical);
    } else if let (Some(parent), Some(name)) = (path.parent(), path.file_name()) {
        if let Ok(canonical_parent) = std::fs::canonicalize(parent) {
            candidates.push(canonical_parent.join(name));
        }
    }
    candidates
}
