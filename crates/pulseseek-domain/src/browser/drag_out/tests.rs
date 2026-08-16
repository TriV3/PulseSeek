use std::path::{Path, PathBuf};

use super::*;

#[derive(Clone, Copy)]
enum FakeOutcome {
    Ok,
    Cancelled,
    Unsupported,
    NotFound,
}

fn outcome(result: FakeOutcome) -> Result<(), DragOutError> {
    match result {
        FakeOutcome::Ok => Ok(()),
        FakeOutcome::Cancelled => Err(DragOutError::cancelled()),
        FakeOutcome::Unsupported => Err(DragOutError::unsupported()),
        FakeOutcome::NotFound => Err(DragOutError::from_io_error(
            std::io::Error::new(std::io::ErrorKind::NotFound, "missing"),
            Path::new("/test/a.wav"),
        )),
    }
}

struct FakeDragOut {
    outcome: FakeOutcome,
}

impl DragOut for FakeDragOut {
    fn drag_out(&self, _paths: &[PathBuf]) -> Result<(), DragOutError> {
        outcome(self.outcome)
    }
}

#[test]
fn drag_out_error_implements_error_contract() {
    let err = DragOutError::from_io_error(
        std::io::Error::new(std::io::ErrorKind::NotFound, "missing"),
        Path::new("/test/a.wav"),
    );
    let desc = err.user_descriptor();
    assert!(!desc.message().is_empty(), "error should have a safe message");
    assert_eq!(err.diagnostic_context().code(), "file.operation");
}

#[test]
fn drag_out_error_not_found_category() {
    let err = DragOutError::from_io_error(
        std::io::Error::new(std::io::ErrorKind::NotFound, "missing"),
        Path::new("/test/a.wav"),
    );
    assert_eq!(err.user_descriptor().category(), ErrorCategory::NotFound);
}

#[test]
fn drag_out_error_permission_denied_category() {
    let err = DragOutError::from_io_error(
        std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
        Path::new("/test/a.wav"),
    );
    assert_eq!(err.user_descriptor().category(), ErrorCategory::PermissionDenied);
}

#[test]
fn drag_out_error_other_errors_are_unavailable() {
    let err = DragOutError::from_io_error(
        std::io::Error::other("launch failed"),
        Path::new("/test/a.wav"),
    );
    assert_eq!(err.user_descriptor().category(), ErrorCategory::Unavailable);
}

#[test]
fn drag_out_error_empty_selection_is_invalid_input() {
    let err = DragOutError::empty_selection();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::InvalidInput);
}

#[test]
fn drag_out_error_cancelled_category() {
    let err = DragOutError::cancelled();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::Cancelled);
}

#[test]
fn drag_out_error_unsupported_category() {
    let err = DragOutError::unsupported();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::Unsupported);
}

#[test]
fn drag_out_success() {
    let drag = FakeDragOut { outcome: FakeOutcome::Ok };
    assert!(drag.drag_out(&[PathBuf::from("/test/a.wav")]).is_ok());
}

#[test]
fn drag_out_propagates_cancelled() {
    let drag = FakeDragOut { outcome: FakeOutcome::Cancelled };
    let err = drag.drag_out(&[PathBuf::from("/test/a.wav")]).unwrap_err();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::Cancelled);
}

#[test]
fn drag_out_propagates_unsupported() {
    let drag = FakeDragOut { outcome: FakeOutcome::Unsupported };
    let err = drag.drag_out(&[PathBuf::from("/test/a.wav")]).unwrap_err();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::Unsupported);
}

#[test]
fn drag_out_propagates_not_found() {
    let drag = FakeDragOut { outcome: FakeOutcome::NotFound };
    let err = drag.drag_out(&[PathBuf::from("/test/a.wav")]).unwrap_err();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::NotFound);
}

#[test]
fn drag_out_accepts_multiple_paths() {
    let drag = FakeDragOut { outcome: FakeOutcome::Ok };
    let paths = vec![PathBuf::from("/test/a.wav"), PathBuf::from("/test/b.wav")];
    assert!(drag.drag_out(&paths).is_ok());
}
