use std::path::{Path, PathBuf};

use super::*;

#[derive(Clone, Copy)]
enum FakeOutcome {
    Ok,
    Unsupported,
    NotFound,
}

fn outcome(result: FakeOutcome) -> Result<(), ExternalActionError> {
    match result {
        FakeOutcome::Ok => Ok(()),
        FakeOutcome::Unsupported => Err(ExternalActionError::unsupported()),
        FakeOutcome::NotFound => Err(ExternalActionError::from_io_error(
            std::io::Error::new(std::io::ErrorKind::NotFound, "missing"),
            Path::new("/test/a.wav"),
        )),
    }
}

struct FakeExternalActions {
    reveal: FakeOutcome,
    open: FakeOutcome,
}

impl ExternalActions for FakeExternalActions {
    fn reveal(&self, _path: &Path) -> Result<(), ExternalActionError> {
        outcome(self.reveal)
    }

    fn open_with(&self, _path: &Path) -> Result<(), ExternalActionError> {
        outcome(self.open)
    }
}

#[test]
fn external_action_error_implements_error_contract() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
    let err = ExternalActionError::from_io_error(io_err, Path::new("/test/a.wav"));
    let desc = err.user_descriptor();
    assert!(!desc.message().is_empty(), "error should have a safe message");
    assert_eq!(err.diagnostic_context().code(), "file.operation");
}

#[test]
fn external_action_error_not_found_category() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
    let err = ExternalActionError::from_io_error(io_err, Path::new("/test/a.wav"));
    assert_eq!(err.user_descriptor().category(), ErrorCategory::NotFound);
}

#[test]
fn external_action_error_permission_denied_category() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
    let err = ExternalActionError::from_io_error(io_err, Path::new("/test/a.wav"));
    assert_eq!(err.user_descriptor().category(), ErrorCategory::PermissionDenied);
}

#[test]
fn external_action_error_unsupported_category() {
    let err = ExternalActionError::unsupported();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::Unsupported);
}

#[test]
fn external_action_error_other_errors_are_unavailable() {
    let io_err = std::io::Error::other("launch failed");
    let err = ExternalActionError::from_io_error(io_err, Path::new("/test/a.wav"));
    assert_eq!(err.user_descriptor().category(), ErrorCategory::Unavailable);
}

#[test]
fn external_actions_reveal_success() {
    let actions = FakeExternalActions { reveal: FakeOutcome::Ok, open: FakeOutcome::Ok };
    assert!(actions.reveal(Path::new("/test/a.wav")).is_ok());
}

#[test]
fn external_actions_reveal_propagates_error() {
    let actions = FakeExternalActions { reveal: FakeOutcome::Unsupported, open: FakeOutcome::Ok };
    let err = actions.reveal(Path::new("/test/a.wav")).unwrap_err();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::Unsupported);
}

#[test]
fn external_actions_open_with_success() {
    let actions = FakeExternalActions { reveal: FakeOutcome::Ok, open: FakeOutcome::Ok };
    assert!(actions.open_with(Path::new("/test/a.wav")).is_ok());
}

#[test]
fn external_actions_open_with_propagates_error() {
    let actions = FakeExternalActions { reveal: FakeOutcome::Ok, open: FakeOutcome::NotFound };
    let err = actions.open_with(Path::new("/test/a.wav")).unwrap_err();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::NotFound);
}

#[test]
fn external_actions_accepts_path_buf() {
    let actions = FakeExternalActions { reveal: FakeOutcome::Ok, open: FakeOutcome::Ok };
    let path = PathBuf::from("/test/a.wav");
    assert!(actions.reveal(&path).is_ok());
    assert!(actions.open_with(&path).is_ok());
}
