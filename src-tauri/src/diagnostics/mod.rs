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

#[cfg(test)]
mod tests;
