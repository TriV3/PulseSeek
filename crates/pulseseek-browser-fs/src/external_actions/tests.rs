use std::path::PathBuf;

use pulseseek_domain::error::ErrorCategory;
use pulseseek_domain::error::ErrorContract;

use super::*;

fn missing_path() -> PathBuf {
    std::env::temp_dir().join(format!("pulseseek-missing-{}", std::process::id()))
}

#[test]
fn reveal_missing_file_returns_not_found_without_launching() {
    let actions = NativeFileActions;
    let err = actions.reveal(&missing_path()).unwrap_err();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::NotFound);
}

#[test]
fn open_with_missing_file_returns_not_found_without_launching() {
    let actions = NativeFileActions;
    let err = actions.open_with(&missing_path()).unwrap_err();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::NotFound);
}

#[test]
fn ensure_exists_accepts_existing_file() {
    let path = std::env::temp_dir().join(format!("pulseseek-existing-{}.tmp", std::process::id()));
    std::fs::write(&path, b"probe").expect("write probe file");
    assert!(ensure_exists(&path).is_ok());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn adapter_errors_use_file_operation_diagnostic_code() {
    let err = ExternalActionError::from_io_error(
        std::io::Error::new(std::io::ErrorKind::NotFound, "missing"),
        Path::new("/test/a.wav"),
    );
    assert_eq!(err.diagnostic_context().code(), "file.operation");
}
