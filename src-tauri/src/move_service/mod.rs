use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use pulseseek_cache::waveform_cache::{waveform_cache_key, WaveformCachePort, WaveformIdentity};
use pulseseek_domain::browser::move_file::FileMove;
use pulseseek_domain::error::{
    ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory, ErrorContract,
};

use crate::playback_events::{
    MoveItemResultData, MoveProgressPayload, PlaybackEventEmitter, EVENT_MOVE_PROGRESS,
};

/// Registers active move sessions and their cancellation flags. Owned by the
/// native move service so `cancel_move_files` can stop a running batch.
#[derive(Clone)]
pub struct ActiveMoves {
    sessions: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl ActiveMoves {
    pub fn new() -> Self {
        Self { sessions: Arc::new(Mutex::new(HashMap::new())) }
    }

    /// Registers a session and returns its cancellation flag.
    pub fn register(&self, session_id: &str) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        self.sessions
            .lock()
            .expect("sessions mutex poisoned")
            .insert(session_id.to_string(), Arc::clone(&flag));
        flag
    }

    /// Sets the cancellation flag for a session. No-op when unknown.
    pub fn cancel(&self, session_id: &str) {
        if let Some(flag) = self.sessions.lock().expect("sessions mutex poisoned").get(session_id) {
            flag.store(true, Ordering::Release);
        }
    }

    /// Removes a session after its move batch finishes.
    pub fn remove(&self, session_id: &str) {
        self.sessions.lock().expect("sessions mutex poisoned").remove(session_id);
    }
}

impl Default for ActiveMoves {
    fn default() -> Self {
        Self::new()
    }
}

/// Application service for moving files with progress and cancellation.
pub trait MoveService: Send {
    /// Validates the target directory, registers a cancellable session, and
    /// spawns a worker that moves every file and emits progress events.
    fn start_move(
        &self,
        paths: Vec<String>,
        target_dir: String,
        events: Arc<dyn PlaybackEventEmitter>,
    ) -> Result<String, ApplicationError>;

    /// Requests cancellation of a running batch. No-op when the session is
    /// unknown or already finished.
    fn cancel_move(&self, session_id: &str);
}

/// Reconcile callback: reports the old and new path of a moved file so the
/// playback service can keep its tracked path consistent (FR-FM-009).
type ReconcileFn = Arc<dyn Fn(&str, &str) -> Result<bool, ApplicationError> + Send + Sync>;

/// Native move service. Runs each batch on its own worker thread so a slow
/// volume never blocks the command loop; per-file progress is streamed as
/// versioned `browser:move-progress` events. A moved playing file is
/// reconciled through the injected callback, and waveform cache rows derived
/// from moved-away paths are invalidated proactively; neither failure ever
/// fails the move itself.
pub struct NativeMoveService<T: FileMove + Send + Sync + 'static> {
    inner: Arc<T>,
    cache: Option<Arc<dyn WaveformCachePort>>,
    reconcile: Option<ReconcileFn>,
    active: ActiveMoves,
    next_session_id: AtomicU64,
}

impl<T: FileMove + Send + Sync + 'static> NativeMoveService<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner: Arc::new(inner),
            cache: None,
            reconcile: None,
            active: ActiveMoves::new(),
            next_session_id: AtomicU64::new(0),
        }
    }

    pub fn with_cache(mut self, cache: Option<Arc<dyn WaveformCachePort>>) -> Self {
        self.cache = cache;
        self
    }

    #[allow(clippy::type_complexity)]
    pub fn with_reconcile(mut self, reconcile: Option<ReconcileFn>) -> Self {
        self.reconcile = reconcile;
        self
    }
}

impl<T: FileMove + Send + Sync + 'static> MoveService for NativeMoveService<T> {
    fn start_move(
        &self,
        paths: Vec<String>,
        target_dir: String,
        events: Arc<dyn PlaybackEventEmitter>,
    ) -> Result<String, ApplicationError> {
        crate::path_validation::validate_directory(&target_dir)?;
        if paths.is_empty() {
            return Err(ApplicationError::new(
                ErrorCategory::InvalidInput,
                DiagnosticContext::new(DiagnosticCode::FileOperation),
                std::io::Error::other("no files selected for move"),
            ));
        }
        let session_id = format!("move-{}", self.next_session_id.fetch_add(1, Ordering::Relaxed));
        let cancelled = self.active.register(&session_id);
        let active_for_thread = self.active.clone();
        let session_for_thread = session_id.clone();
        let inner = Arc::clone(&self.inner);
        let cache = self.cache.clone();
        let reconcile = self.reconcile.clone();
        let path_bufs: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
        let target_dir_buf = PathBuf::from(&target_dir);

        std::thread::Builder::new()
            .name("pulseseek-file-move".to_string())
            .spawn(move || {
                let results = inner.move_files(&path_bufs, &target_dir_buf, &cancelled);
                let total = results.len();
                let mut processed: Vec<MoveItemResultData> = Vec::with_capacity(total);
                for (index, (path, result)) in results.iter().enumerate() {
                    let item = match result {
                        Ok(new_path) => {
                            let old = path.to_string_lossy().to_string();
                            let new = new_path.to_string_lossy().to_string();
                            if let Some(reconcile) = reconcile.as_ref() {
                                if let Err(error) = reconcile(&old, &new) {
                                    tracing::warn!(
                                        error = %error,
                                        "playback reconcile after move failed"
                                    );
                                }
                            }
                            if let Some(cache) = cache.as_ref() {
                                invalidate_cache(&**cache, path);
                            }
                            MoveItemResultData {
                                path: old,
                                new_path: Some(new),
                                ok: true,
                                category: None,
                                message: None,
                                diagnostic_code: None,
                            }
                        },
                        Err(error) => {
                            let descriptor = error.user_descriptor();
                            MoveItemResultData {
                                path: path.to_string_lossy().to_string(),
                                new_path: None,
                                ok: false,
                                category: Some(format!("{:?}", descriptor.category())),
                                message: Some(descriptor.message().to_string()),
                                diagnostic_code: Some(
                                    error.diagnostic_context().code().to_string(),
                                ),
                            }
                        },
                    };
                    processed.push(item);
                    if !events.is_disconnected() {
                        // Intermediate events carry only progress; the full
                        // results array ships on the final done event so a
                        // large batch never sends O(N²) payload data.
                        let mut payload_results: &[MoveItemResultData] = &[];
                        if index + 1 == total {
                            payload_results = &processed;
                        }
                        emit_move_progress(
                            &*events,
                            &session_for_thread,
                            payload_results,
                            index + 1,
                            total,
                        );
                    }
                }
                // Safety net: an empty batch still exits the progress state.
                if processed.is_empty() && !events.is_disconnected() {
                    emit_move_progress(&*events, &session_for_thread, &[], 0, total);
                }
                active_for_thread.remove(&session_for_thread);
            })
            .map_err(|error| {
                self.active.remove(&session_id);
                ApplicationError::new(
                    ErrorCategory::Unavailable,
                    DiagnosticContext::new(DiagnosticCode::FileOperation),
                    error,
                )
            })?;
        Ok(session_id)
    }

    fn cancel_move(&self, session_id: &str) {
        self.active.cancel(session_id);
    }
}

/// Emits one `browser:move-progress` event. `done` is true when `completed`
/// reached `total`, which also marks the batch as finished.
pub(crate) fn emit_move_progress(
    events: &dyn PlaybackEventEmitter,
    session_id: &str,
    results: &[MoveItemResultData],
    completed: usize,
    total: usize,
) {
    let payload = MoveProgressPayload {
        session_id: session_id.to_string(),
        completed,
        total,
        done: completed == total,
        results: results.to_vec(),
    };
    let _ = events.emit(
        EVENT_MOVE_PROGRESS,
        serde_json::to_value(payload).expect("move progress serialization"),
    );
}

/// Deletes waveform rows stored under a moved-away path (raw and
/// canonicalized candidates, matching the file watcher's scheme). A cache
/// failure only logs and never blocks the move.
fn invalidate_cache(cache: &dyn WaveformCachePort, old_path: &Path) {
    for candidate in cache_key_candidates(old_path) {
        let key = waveform_cache_key(&WaveformIdentity::new(candidate, 0, 0));
        if let Err(error) = cache.delete_waveform(&key) {
            tracing::warn!(error = %error, "waveform cache invalidation failed");
        }
    }
}

/// Path forms that may have produced the stored cache key, mirroring the
/// rename service and the file watcher.
fn cache_key_candidates(path: &Path) -> Vec<PathBuf> {
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

/// In-memory move service used by command-layer tests.
#[allow(clippy::type_complexity)]
pub struct FakeMoveService {
    f: Box<
        dyn Fn(
                Vec<String>,
                String,
                Arc<dyn PlaybackEventEmitter>,
            ) -> Result<String, ApplicationError>
            + Send
            + Sync,
    >,
}

impl FakeMoveService {
    #[allow(clippy::type_complexity)]
    pub fn new(
        f: Box<
            dyn Fn(
                    Vec<String>,
                    String,
                    Arc<dyn PlaybackEventEmitter>,
                ) -> Result<String, ApplicationError>
                + Send
                + Sync,
        >,
    ) -> Self {
        Self { f }
    }
}

impl MoveService for FakeMoveService {
    fn start_move(
        &self,
        paths: Vec<String>,
        target_dir: String,
        events: Arc<dyn PlaybackEventEmitter>,
    ) -> Result<String, ApplicationError> {
        (self.f)(paths, target_dir, events)
    }

    fn cancel_move(&self, _session_id: &str) {}
}

#[cfg(test)]
mod tests;
