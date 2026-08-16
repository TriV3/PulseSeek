use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use pulseseek_domain::error::ErrorCategory;
use pulseseek_domain::error::ErrorContract;

use super::*;

fn missing_path() -> PathBuf {
    std::env::temp_dir().join(format!("pulseseek-dragout-missing-{}", std::process::id()))
}

#[derive(Clone, Copy)]
enum FakeOutcome {
    Ok,
    Cancelled,
    NotFound,
    Unsupported,
}

impl FakeOutcome {
    fn result(self) -> Result<(), DragOutError> {
        match self {
            FakeOutcome::Ok => Ok(()),
            FakeOutcome::Cancelled => Err(DragOutError::cancelled()),
            FakeOutcome::NotFound => Err(DragOutError::from_io_error(
                std::io::Error::new(std::io::ErrorKind::NotFound, "missing"),
                std::path::Path::new("/test/a.wav"),
            )),
            FakeOutcome::Unsupported => Err(DragOutError::unsupported()),
        }
    }
}

struct RecordingStarter {
    calls: AtomicUsize,
    outcome: FakeOutcome,
}

impl RecordingStarter {
    fn new(outcome: FakeOutcome) -> Self {
        Self { calls: AtomicUsize::new(0), outcome }
    }

    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl DragStarter for RecordingStarter {
    fn start(&self, _paths: &[PathBuf]) -> Result<(), DragOutError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.outcome.result()
    }
}

#[test]
fn drag_out_empty_selection_is_invalid_input_without_starting() {
    let starter = RecordingStarter::new(FakeOutcome::Ok);
    let drag = NativeDragOut::new(starter);
    let err = drag.drag_out(&[]).unwrap_err();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::InvalidInput);
    assert_eq!(drag.starter.call_count(), 0, "starter must not run for an empty selection");
}

#[test]
fn drag_out_missing_file_returns_not_found_without_starting() {
    let starter = RecordingStarter::new(FakeOutcome::Ok);
    let drag = NativeDragOut::new(starter);
    let err = drag.drag_out(&[missing_path()]).unwrap_err();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::NotFound);
    assert_eq!(drag.starter.call_count(), 0, "starter must not run for a missing target");
}

#[test]
fn drag_out_missing_file_in_multi_selection_returns_not_found() {
    let starter = RecordingStarter::new(FakeOutcome::Ok);
    let drag = NativeDragOut::new(starter);
    let existing =
        std::env::temp_dir().join(format!("pulseseek-dragout-existing-{}.tmp", std::process::id()));
    std::fs::write(&existing, b"probe").expect("write probe file");
    let err = drag.drag_out(&[existing.clone(), missing_path()]).unwrap_err();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::NotFound);
    assert_eq!(drag.starter.call_count(), 0, "starter must not run when any target is missing");
    let _ = std::fs::remove_file(&existing);
}

#[test]
fn drag_out_delegates_to_starter_when_all_paths_exist() {
    let starter = RecordingStarter::new(FakeOutcome::Ok);
    let drag = NativeDragOut::new(starter);
    let a = std::env::temp_dir().join(format!("pulseseek-dragout-a-{}.tmp", std::process::id()));
    let b = std::env::temp_dir().join(format!("pulseseek-dragout-b-{}.tmp", std::process::id()));
    std::fs::write(&a, b"a").expect("write probe file");
    std::fs::write(&b, b"b").expect("write probe file");
    assert!(drag.drag_out(&[a.clone(), b.clone()]).is_ok());
    assert_eq!(drag.starter.call_count(), 1, "starter must run once for a valid selection");
    let _ = std::fs::remove_file(&a);
    let _ = std::fs::remove_file(&b);
}

#[test]
fn drag_out_propagates_starter_cancellation() {
    let starter = RecordingStarter::new(FakeOutcome::Cancelled);
    let drag = NativeDragOut::new(starter);
    let a = std::env::temp_dir().join(format!("pulseseek-dragout-c-{}.tmp", std::process::id()));
    std::fs::write(&a, b"c").expect("write probe file");
    let err = drag.drag_out(std::slice::from_ref(&a)).unwrap_err();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::Cancelled);
    let _ = std::fs::remove_file(&a);
}

#[test]
fn drag_out_propagates_starter_not_found() {
    let starter = RecordingStarter::new(FakeOutcome::NotFound);
    let drag = NativeDragOut::new(starter);
    let a = std::env::temp_dir().join(format!("pulseseek-dragout-nf-{}.tmp", std::process::id()));
    std::fs::write(&a, b"nf").expect("write probe file");
    let err = drag.drag_out(std::slice::from_ref(&a)).unwrap_err();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::NotFound);
    let _ = std::fs::remove_file(&a);
}

#[test]
fn drag_out_propagates_starter_unsupported() {
    let starter = RecordingStarter::new(FakeOutcome::Unsupported);
    let drag = NativeDragOut::new(starter);
    let a = std::env::temp_dir().join(format!("pulseseek-dragout-us-{}.tmp", std::process::id()));
    std::fs::write(&a, b"us").expect("write probe file");
    let err = drag.drag_out(std::slice::from_ref(&a)).unwrap_err();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::Unsupported);
    let _ = std::fs::remove_file(&a);
}

#[test]
fn drag_out_errors_use_file_operation_diagnostic_code() {
    let err = DragOutError::from_io_error(
        std::io::Error::new(std::io::ErrorKind::NotFound, "missing"),
        std::path::Path::new("/test/a.wav"),
    );
    assert_eq!(err.diagnostic_context().code(), "file.operation");
}
