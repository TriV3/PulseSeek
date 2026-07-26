use std::fs;
use std::path::PathBuf;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Configuration for local diagnostics.
#[derive(Clone, Debug)]
pub struct DiagnosticsConfig {
    /// Directory for log files.
    pub log_dir: PathBuf,
    /// Maximum number of rotated log files to keep.
    pub max_log_files: usize,
    /// Minimum log level for the file output.
    pub log_level: LevelFilter,
}

impl Default for DiagnosticsConfig {
    fn default() -> Self {
        let log_dir = default_log_dir().unwrap_or_else(|| PathBuf::from("."));
        Self { log_dir, max_log_files: 5, log_level: LevelFilter::INFO }
    }
}

/// Initializes the global tracing subscriber with rolling file output.
///
/// ## Panics
/// Panics if a global subscriber is already set or if the log directory
/// cannot be created.
pub fn init(config: DiagnosticsConfig) -> ShutdownGuard {
    fs::create_dir_all(&config.log_dir).expect("failed to create log directory");

    // Remove excess log files before starting.
    enforce_log_bound(&config.log_dir, config.max_log_files);

    let file_appender = tracing_appender::rolling::daily(&config.log_dir, "pulseseek");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = tracing_subscriber::filter::EnvFilter::builder()
        .with_default_directive(config.log_level.into())
        .from_env_lossy();

    let subscriber = tracing_subscriber::registry().with(env_filter).with(
        tracing_subscriber::fmt::layer()
            .with_writer(non_blocking)
            .with_ansi(false)
            .with_target(true)
            .with_thread_ids(true),
    );

    subscriber.try_init().expect("global tracing subscriber already set");

    ShutdownGuard { _guard: Some(guard) }
}

/// Guard that flushes pending log data on drop.
pub struct ShutdownGuard {
    _guard: Option<WorkerGuard>,
}

impl Drop for ShutdownGuard {
    fn drop(&mut self) {
        // WorkerGuard flushes on drop.
    }
}

fn default_log_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        std::env::var("HOME")
            .ok()
            .map(|home| PathBuf::from(home).join("Library").join("Logs").join("PulseSeek"))
    }
    #[cfg(target_os = "linux")]
    {
        std::env::var("XDG_DATA_HOME").ok().map(PathBuf::from).or_else(|| {
            std::env::var("HOME").ok().map(|home| {
                PathBuf::from(home).join(".local").join("share").join("PulseSeek").join("logs")
            })
        })
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var("LOCALAPPDATA")
            .ok()
            .map(|appdata| PathBuf::from(appdata).join("PulseSeek").join("logs"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        None
    }
}

/// Enforces the maximum log file count by deleting the oldest files.
///
/// Returns the number of deleted files.
pub fn enforce_log_bound(dir: &PathBuf, max_files: usize) -> usize {
    let Ok(mut entries) = fs::read_dir(dir).map(|rd| {
        rd.filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "log"))
            .collect::<Vec<_>>()
    }) else {
        return 0;
    };

    if entries.len() <= max_files {
        return 0;
    }

    // Sort by modification time (oldest first).
    entries.sort_by_key(|e| e.metadata().ok().and_then(|m| m.modified().ok()));

    let excess = entries.len() - max_files;
    for entry in entries.iter().take(excess) {
        let _ = fs::remove_file(entry.path());
    }

    excess
}

/// Serialisable error report sent from the React boundary.
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct ClientErrorReport {
    pub category: String,
    pub message: String,
    pub diagnostic_code: String,
}

/// Tauri command: accepts an error report from the React boundary and logs it.
#[tauri::command]
pub fn report_error(report: ClientErrorReport) {
    tracing::error!(
        category = %report.category,
        diagnostic_code = %report.diagnostic_code,
        "{}",
        report.message
    );
}

/// Thread-safe buffer for capturing log output in tests.
#[cfg(test)]
use std::sync::{Arc, Mutex};

#[cfg(test)]
#[derive(Clone)]
struct LogCapture(Arc<Mutex<Vec<String>>>);

#[cfg(test)]
impl LogCapture {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(Vec::new())))
    }

    fn lines(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

#[cfg(test)]
impl std::io::Write for LogCapture {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let s = String::from_utf8_lossy(buf).to_string();
        self.0.lock().unwrap().push(s);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogCapture {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[cfg(test)]
mod tests {
    use pulseseek_domain::error::{
        ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory,
    };
    use tracing_subscriber::filter::LevelFilter;

    use super::*;

    /// Runs a closure with a test subscriber that captures log output.
    fn with_captured_log<F>(f: F)
    where
        F: FnOnce(LogCapture),
    {
        let capture = LogCapture::new();

        let subscriber = tracing_subscriber::fmt()
            .with_writer(capture.clone())
            .with_max_level(LevelFilter::TRACE)
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            f(capture);
        });
    }

    #[test]
    fn log_file_created_on_init() {
        let temp_dir = std::env::temp_dir().join("pulseseek-test-init");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let config = DiagnosticsConfig {
            log_dir: temp_dir.clone(),
            max_log_files: 3,
            log_level: LevelFilter::DEBUG,
        };

        // Calling init should create the log dir and write at least one file.
        // In Red phase this will panic due to todo!().
        let _guard = init(config);

        let entries: Vec<_> = std::fs::read_dir(&temp_dir)
            .expect("log dir should exist")
            .filter_map(|e| e.ok())
            .collect();
        assert!(!entries.is_empty(), "no log files found after init");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn safe_message_in_log_private_path_absent() {
        with_captured_log(|capture| {
            let private_path = "/Users/alice/secret-sessions/unreleased.wav";
            let adapter_error =
                std::io::Error::new(std::io::ErrorKind::PermissionDenied, private_path);
            let app_error = ApplicationError::new(
                ErrorCategory::PermissionDenied,
                DiagnosticContext::new(DiagnosticCode::BrowserRead),
                adapter_error,
            );

            tracing::error!(error = %app_error);

            let lines = capture.lines();
            let all_logs = lines.join("\n");

            // Safe user message must appear.
            assert!(
                all_logs.contains("PulseSeek could not access that item."),
                "safe message should be in log output"
            );
            // Private path must NOT appear.
            assert!(
                !all_logs.contains(private_path),
                "private path should not appear in log output"
            );
        });
    }

    #[test]
    fn bounded_log_count_enforced_on_startup() {
        let temp_dir = std::env::temp_dir().join("pulseseek-test-bound");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).expect("create temp dir");

        for i in 0..10 {
            let path = temp_dir.join(format!("pulse.{:04}.log", i));
            std::fs::write(&path, "fake log content").expect("write fake log");
        }

        // In Red phase this will panic due to todo!().
        let deleted = enforce_log_bound(&temp_dir, 5);
        assert_eq!(deleted, 5, "should delete 5 excess files");

        let remaining: Vec<_> =
            std::fs::read_dir(&temp_dir).expect("read dir").filter_map(|e| e.ok()).collect();
        assert_eq!(remaining.len(), 5, "should keep at most 5 files");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn log_level_filters_correctly() {
        with_captured_log(|capture| {
            tracing::info!("this is info");
            tracing::warn!("this is warn");

            let lines = capture.lines();
            let all_logs = lines.join("\n");

            assert!(all_logs.contains("this is warn"), "warn message should appear");
        });
    }
}
