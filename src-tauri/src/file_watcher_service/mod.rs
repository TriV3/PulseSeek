use pulseseek_domain::error::{ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory};

/// Watches one folder for external filesystem changes and signals a refresh.
///
/// Only a single folder is watched at a time (single active watch): starting a
/// watch replaces the previous one. Detected changes are debounced into one
/// `browser:file-change` event so bursts do not spam the frontend, and modified
/// sources have their cached waveform invalidated (FR-BR-008, FR-FM-010).
pub trait FileWatcherService: Send {
    /// Watches `path`, replacing any previously watched folder.
    fn start_watching(&mut self, path: &str) -> Result<(), ApplicationError>;

    /// Stops watching the current folder, if any.
    fn stop_watching(&mut self) -> Result<(), ApplicationError>;

    /// Returns the currently watched folder path, if any.
    fn watched_path(&self) -> Option<String>;
}

/// In-memory watcher used by command-layer tests.
#[derive(Default)]
pub struct FakeFileWatcherService {
    /// Every folder passed to `start_watching`, in call order.
    pub watch_calls: Vec<String>,
    /// Number of `stop_watching` calls.
    pub stop_calls: usize,
    /// When true, `start_watching` fails with an unavailable error.
    pub fail_watch: bool,
    /// The currently watched folder, mirrored by the fake.
    pub watched: Option<String>,
}

impl FakeFileWatcherService {
    pub fn new() -> Self {
        Self::default()
    }
}

impl FileWatcherService for FakeFileWatcherService {
    fn start_watching(&mut self, path: &str) -> Result<(), ApplicationError> {
        if self.fail_watch {
            return Err(ApplicationError::new(
                ErrorCategory::Unavailable,
                DiagnosticContext::new(DiagnosticCode::BrowserRead),
                std::io::Error::other("fake file watcher failure"),
            ));
        }
        self.watch_calls.push(path.to_string());
        self.watched = Some(path.to_string());
        Ok(())
    }

    fn stop_watching(&mut self) -> Result<(), ApplicationError> {
        self.stop_calls += 1;
        self.watched = None;
        Ok(())
    }

    fn watched_path(&self) -> Option<String> {
        self.watched.clone()
    }
}

mod native;
pub(crate) use native::*;

#[cfg(test)]
mod tests;
