use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use pulseseek_domain::browser::copy_file::FileCopy;
use pulseseek_domain::error::{
    ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory, ErrorContract,
};

use crate::playback_events::{
    CopyItemResultData, CopyProgressPayload, PlaybackEventEmitter, EVENT_COPY_PROGRESS,
};

/// Registers active copy sessions and their cancellation flags. Owned by the
/// native copy service so `cancel_copy_files` can stop a running batch.
#[derive(Clone)]
pub struct ActiveCopies {
    sessions: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl ActiveCopies {
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

    /// Removes a session after its copy batch finishes.
    pub fn remove(&self, session_id: &str) {
        self.sessions.lock().expect("sessions mutex poisoned").remove(session_id);
    }
}

impl Default for ActiveCopies {
    fn default() -> Self {
        Self::new()
    }
}

/// Port for the copy application service. Kept as a trait so the command
/// layer can inject an in-memory fake and the native worker stays testable.
pub trait CopyService: Send + Sync {
    /// Starts a copy batch for `paths` into `target_dir` on a worker thread
    /// and returns a session id. Progress streams through
    /// `browser:copy-progress` events.
    fn start_copy(
        &self,
        paths: Vec<String>,
        target_dir: String,
        events: Arc<dyn PlaybackEventEmitter>,
    ) -> Result<String, ApplicationError>;

    /// Requests cancellation of a running batch. No-op when the session is
    /// unknown or already finished.
    fn cancel_copy(&self, session_id: &str);
}

/// Native copy service. Runs each batch on its own worker thread so a slow
/// volume never blocks the command loop; per-file progress is streamed as
/// versioned `browser:copy-progress` events. Copying never modifies the
/// source, so there is no playback reconcile or waveform-cache invalidation:
/// the source's cached waveform stays valid and the new copy simply has no
/// cached row yet.
pub struct NativeCopyService<T: FileCopy + Send + Sync + 'static> {
    inner: Arc<T>,
    active: ActiveCopies,
    next_session_id: AtomicU64,
}

impl<T: FileCopy + Send + Sync + 'static> NativeCopyService<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner: Arc::new(inner),
            active: ActiveCopies::new(),
            next_session_id: AtomicU64::new(0),
        }
    }
}

impl<T: FileCopy + Send + Sync + 'static> CopyService for NativeCopyService<T> {
    fn start_copy(
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
                std::io::Error::other("no files selected for copy"),
            ));
        }
        let session_id = format!("copy-{}", self.next_session_id.fetch_add(1, Ordering::Relaxed));
        let cancelled = self.active.register(&session_id);
        let active_for_thread = self.active.clone();
        let session_for_thread = session_id.clone();
        let inner = Arc::clone(&self.inner);
        let path_bufs: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
        let target_dir_buf = PathBuf::from(&target_dir);

        std::thread::Builder::new()
            .name("pulseseek-file-copy".to_string())
            .spawn(move || {
                let results = inner.copy_files(&path_bufs, &target_dir_buf, &cancelled);
                let total = results.len();
                let mut processed: Vec<CopyItemResultData> = Vec::with_capacity(total);
                for (index, (path, result)) in results.iter().enumerate() {
                    let item = match result {
                        Ok(new_path) => CopyItemResultData {
                            path: path.to_string_lossy().to_string(),
                            new_path: Some(new_path.to_string_lossy().to_string()),
                            ok: true,
                            category: None,
                            message: None,
                            diagnostic_code: None,
                        },
                        Err(error) => {
                            let descriptor = error.user_descriptor();
                            CopyItemResultData {
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
                        let mut payload_results: &[CopyItemResultData] = &[];
                        if index + 1 == total {
                            payload_results = &processed;
                        }
                        emit_copy_progress(
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
                    emit_copy_progress(&*events, &session_for_thread, &[], 0, total);
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

    fn cancel_copy(&self, session_id: &str) {
        self.active.cancel(session_id);
    }
}

/// Emits one `browser:copy-progress` event. `done` is true when `completed`
/// reached `total`, which also marks the batch as finished.
pub(crate) fn emit_copy_progress(
    events: &dyn PlaybackEventEmitter,
    session_id: &str,
    results: &[CopyItemResultData],
    completed: usize,
    total: usize,
) {
    let payload = CopyProgressPayload {
        session_id: session_id.to_string(),
        completed,
        total,
        done: completed == total,
        results: results.to_vec(),
    };
    let _ = events.emit(
        EVENT_COPY_PROGRESS,
        serde_json::to_value(payload).expect("copy progress serialization"),
    );
}

/// In-memory copy service used by command-layer tests.
#[allow(clippy::type_complexity)]
pub struct FakeCopyService {
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

impl FakeCopyService {
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

impl CopyService for FakeCopyService {
    fn start_copy(
        &self,
        paths: Vec<String>,
        target_dir: String,
        events: Arc<dyn PlaybackEventEmitter>,
    ) -> Result<String, ApplicationError> {
        (self.f)(paths, target_dir, events)
    }

    fn cancel_copy(&self, _session_id: &str) {}
}

#[cfg(test)]
mod tests;
