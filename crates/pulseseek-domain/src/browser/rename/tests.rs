use std::path::{Path, PathBuf};

use super::*;

#[allow(clippy::type_complexity)]
struct FakeFileRename {
    f: Box<dyn Fn(&Path, &str) -> Result<PathBuf, RenameError> + Send>,
}

impl FakeFileRename {
    #[allow(clippy::type_complexity)]
    fn new(f: Box<dyn Fn(&Path, &str) -> Result<PathBuf, RenameError> + Send>) -> Self {
        Self { f }
    }
}

impl FileRename for FakeFileRename {
    fn rename(&self, path: &Path, new_name: &str) -> Result<PathBuf, RenameError> {
        (self.f)(path, new_name)
    }
}

#[test]
fn rename_error_implements_error_contract() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "test");
    let err = RenameError::from_io_error(io_err, Path::new("/test"));
    let desc = err.user_descriptor();
    assert!(!desc.message().is_empty(), "error should have a safe message");
    assert_eq!(err.diagnostic_context().code(), "file.operation");
}

#[test]
fn rename_error_permission_denied_category() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
    let err = RenameError::from_io_error(io_err, Path::new("/test"));
    assert_eq!(err.user_descriptor().category(), ErrorCategory::PermissionDenied);
}

#[test]
fn rename_error_not_found_category() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
    let err = RenameError::from_io_error(io_err, Path::new("/test"));
    assert_eq!(err.user_descriptor().category(), ErrorCategory::NotFound);
}

#[test]
fn rename_error_already_exists_is_conflict() {
    let io_err = std::io::Error::new(std::io::ErrorKind::AlreadyExists, "exists");
    let err = RenameError::from_io_error(io_err, Path::new("/test"));
    assert_eq!(err.user_descriptor().category(), ErrorCategory::Conflict);
}

#[test]
fn rename_error_other_errors_are_unavailable() {
    let io_err = std::io::Error::other("other");
    let err = RenameError::from_io_error(io_err, Path::new("/test"));
    assert_eq!(err.user_descriptor().category(), ErrorCategory::Unavailable);
}

#[test]
fn invalid_name_error_has_invalid_input_category() {
    let err = RenameError::invalid_name("name is empty");
    assert_eq!(err.user_descriptor().category(), ErrorCategory::InvalidInput);
    assert_eq!(err.diagnostic_context().code(), "file.operation");
}

#[test]
fn collision_error_has_conflict_category() {
    let err = RenameError::collision();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::Conflict);
}

#[test]
fn validate_rename_name_rejects_empty() {
    let err = validate_rename_name("").expect_err("empty name is invalid");
    assert_eq!(err.user_descriptor().category(), ErrorCategory::InvalidInput);
}

#[test]
fn validate_rename_name_rejects_path_separators() {
    for name in ["a/b", "a\\b", "a/b/c", "\\"] {
        assert!(validate_rename_name(name).is_err(), "path separator should be rejected: {name:?}");
    }
}

#[test]
fn validate_rename_name_rejects_dot_components() {
    for name in [".", ".."] {
        assert!(
            validate_rename_name(name).is_err(),
            "reserved component should be rejected: {name:?}"
        );
    }
}

#[test]
fn validate_rename_name_rejects_nul_byte() {
    assert!(validate_rename_name("bad\0name").is_err(), "NUL byte should be rejected");
}

#[test]
fn validate_rename_name_rejects_overlong_name() {
    let long = "a".repeat(256);
    assert!(validate_rename_name(&long).is_err(), "overlong name should be rejected");
}

#[test]
fn validate_rename_name_accepts_plain_basename() {
    for name in ["track.wav", "Track One.flac", "01 - intro.mp3", "a.b.c.ogg"] {
        assert!(validate_rename_name(name).is_ok(), "valid basename: {name:?}");
    }
}

#[test]
fn validate_rename_name_accepts_max_length_name() {
    let max = "a".repeat(255);
    assert!(validate_rename_name(&max).is_ok(), "max-length name should be accepted");
}

#[test]
fn file_rename_port_returns_new_path() {
    let rename = FakeFileRename::new(Box::new(|path, name| {
        Ok(path.parent().unwrap_or(Path::new("/")).join(name))
    }));
    let result = rename.rename(Path::new("/music/track.wav"), "renamed.wav");
    assert_eq!(result.unwrap(), PathBuf::from("/music/renamed.wav"));
}

#[test]
fn file_rename_port_propagates_error() {
    let rename = FakeFileRename::new(Box::new(|_, _| Err(RenameError::collision())));
    let result = rename.rename(Path::new("/music/track.wav"), "existing.wav");
    assert_eq!(result.unwrap_err().user_descriptor().category(), ErrorCategory::Conflict);
}
