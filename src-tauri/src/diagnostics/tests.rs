use std::sync::{Arc, Mutex};

use super::*;
use pulseseek_domain::error::{ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory};
use tracing_subscriber::filter::LevelFilter;

#[derive(Clone)]
struct LogCapture(Arc<Mutex<Vec<String>>>);

impl LogCapture {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(Vec::new())))
    }

    fn lines(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

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

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogCapture {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

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
        let adapter_error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, private_path);
        let app_error = ApplicationError::new(
            ErrorCategory::PermissionDenied,
            DiagnosticContext::new(DiagnosticCode::BrowserRead),
            adapter_error,
        );
        tracing::error!(error = %app_error);
        let lines = capture.lines();
        let all_logs = lines.join("\n");
        assert!(
            all_logs.contains("PulseSeek could not access that item."),
            "safe message should be in log output"
        );
        assert!(!all_logs.contains(private_path), "private path should not appear in log output");
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
