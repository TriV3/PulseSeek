use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use pulseseek_domain::error::{ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory};

use crate::playback_events::PlaybackEventEmitter;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserRootKind {
    System,
    Home,
    Physical,
    Network,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserLibraryKind {
    Documents,
    Music,
    Pictures,
    Videos,
    Downloads,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BrowserLibraryData {
    pub path: String,
    pub name: String,
    pub kind: BrowserLibraryKind,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BrowserRootData {
    pub path: String,
    pub name: String,
    pub kind: BrowserRootKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FolderEnumerationOptions {
    pub batch_size: usize,
    pub show_unsupported: bool,
    pub recursive: bool,
    pub show_hidden: bool,
}

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
    /// Lists filesystem roots currently available to browse, including mounted
    /// network volumes exposed by the operating system.
    fn list_roots(&self) -> Result<Vec<BrowserRootData>, ApplicationError>;

    fn list_libraries(&self) -> Result<Vec<BrowserLibraryData>, ApplicationError> {
        Ok(native::system_libraries())
    }

    /// Starts enumerating a folder.
    ///
    /// Returns a session_id that can be used to cancel the enumeration.
    /// When `recursive` is true, the walk covers the whole subtree below
    /// `path` with cycle protection instead of a single folder level.
    fn start_enumeration(
        &mut self,
        path: &str,
        options: FolderEnumerationOptions,
        active: &ActiveEnumerations,
        events: Arc<dyn PlaybackEventEmitter>,
    ) -> Result<String, ApplicationError>;

    /// Attaches a file watcher that observes external changes to the browsed
    /// folder (FR-BR-008, FR-FM-010). The default implementation is a no-op
    /// so fake services used in tests do not need to override this.
    fn set_watcher(&mut self, _watcher: Box<dyn crate::file_watcher_service::FileWatcherService>) {}
}

/// Fake implementation of [`FolderEnumerationService`] for tests.
pub struct FakeFolderEnumerationService {
    pub roots: Vec<BrowserRootData>,
    pub libraries: Vec<BrowserLibraryData>,
    pub start_call_count: u64,
    pub last_path: Option<String>,
    pub last_batch_size: Option<usize>,
    pub last_show_unsupported: Option<bool>,
    pub last_recursive: Option<bool>,
    pub last_show_hidden: Option<bool>,
    pub fail_start: bool,
    pub next_session_id: String,
}

impl FakeFolderEnumerationService {
    pub fn new() -> Self {
        Self {
            roots: vec![BrowserRootData {
                path: std::path::MAIN_SEPARATOR.to_string(),
                name: "System".to_string(),
                kind: BrowserRootKind::System,
            }],
            libraries: Vec::new(),
            start_call_count: 0,
            last_path: None,
            last_batch_size: None,
            last_show_unsupported: None,
            last_recursive: None,
            last_show_hidden: None,
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
    fn list_roots(&self) -> Result<Vec<BrowserRootData>, ApplicationError> {
        Ok(self.roots.clone())
    }

    fn list_libraries(&self) -> Result<Vec<BrowserLibraryData>, ApplicationError> {
        Ok(self.libraries.clone())
    }

    fn start_enumeration(
        &mut self,
        path: &str,
        options: FolderEnumerationOptions,
        _active: &ActiveEnumerations,
        _events: Arc<dyn PlaybackEventEmitter>,
    ) -> Result<String, ApplicationError> {
        self.start_call_count += 1;
        self.last_path = Some(path.to_string());
        self.last_batch_size = Some(options.batch_size);
        self.last_show_unsupported = Some(options.show_unsupported);
        self.last_recursive = Some(options.recursive);
        self.last_show_hidden = Some(options.show_hidden);
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

mod native;
pub(crate) use native::*;

#[cfg(test)]
mod tests;
