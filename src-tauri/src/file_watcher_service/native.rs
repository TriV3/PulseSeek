use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify_debouncer_full::notify::{Config, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer_opt, DebounceEventResult, Debouncer, NoCache};
use pulseseek_cache::waveform_cache::{waveform_cache_key, WaveformCachePort, WaveformIdentity};
use pulseseek_domain::error::{ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory};

use crate::playback_events::{FileChangePayload, PlaybackEventEmitter, EVENT_FILE_CHANGE};

use super::FileWatcherService;

/// Default debounce window used to coalesce filesystem events.
pub const DEFAULT_DEBOUNCE_MS: u64 = 200;

/// Native file watcher backed by `notify-debouncer-full`.
///
/// The debouncer runs on its own thread and coalesces bursts. On a debounced
/// batch the service invalidates cached waveforms for modified or removed
/// sources and emits one `browser:file-change` event carrying the watched
/// folder path.
///
/// `NoCache` is used instead of the recommended file-ID cache: the
/// recommended cache walks the whole watched tree on every `watch()` call,
/// which stalls on large folders such as a home directory. This service only
/// needs a coarse "something changed" signal, so rename stitching accuracy is
/// not worth blocking enumeration.
pub struct NativeFileWatcherService {
    debouncer: Mutex<Debouncer<RecommendedWatcher, NoCache>>,
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
        let debouncer = new_debouncer_opt::<_, RecommendedWatcher, NoCache>(
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
            Config::default(),
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

impl FileWatcherService for NativeFileWatcherService {
    fn start_watching(&mut self, path: &str) -> Result<(), ApplicationError> {
        tracing::debug!(path = %path, "file watcher start_watching begin");
        let target = PathBuf::from(path);
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
            tracing::debug!(count = debounced.len(), "file watcher debounced batch");
            for event in &debounced {
                invalidate_changed(&event.event, cache);
            }
            let Some(path) = watched.lock().expect("watched mutex poisoned").clone() else {
                return;
            };
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
