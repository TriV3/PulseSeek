use std::path::Path;

use super::*;

#[test]
fn folder_read_error_implements_error_contract() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "test");
    let err = FolderReadError::from_io_error(io_err, Path::new("/test"));
    let desc = err.user_descriptor();
    assert!(!desc.message().is_empty(), "error should have a safe message");
    assert_eq!(err.diagnostic_context().code(), "browser.read");
}

#[test]
fn folder_read_error_permission_denied_category() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
    let err = FolderReadError::from_io_error(io_err, Path::new("/test"));
    assert_eq!(err.user_descriptor().category(), ErrorCategory::PermissionDenied);
}

#[test]
fn folder_read_error_not_found_category() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
    let err = FolderReadError::from_io_error(io_err, Path::new("/test"));
    assert_eq!(err.user_descriptor().category(), ErrorCategory::NotFound);
}

#[test]
fn folder_read_error_other_errors_are_unavailable() {
    let io_err = std::io::Error::other("other");
    let err = FolderReadError::from_io_error(io_err, Path::new("/test"));
    assert_eq!(err.user_descriptor().category(), ErrorCategory::Unavailable);
}
