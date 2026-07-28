use std::error::Error;
use std::fmt;
use std::path::Path;

use crate::browser::entry::BrowserEntry;
use crate::error::{
    DiagnosticCode, DiagnosticContext, ErrorCategory, ErrorContract, UserErrorDescriptor,
};

/// Error returned by [`FolderReader`] operations.
#[derive(Debug)]
pub struct FolderReadError {
    category: ErrorCategory,
    context: DiagnosticContext,
    source: Box<dyn Error + Send + Sync + 'static>,
}

impl FolderReadError {
    /// Creates a new error from an `std::io::Error` and the path that was
    /// being accessed.
    pub fn from_io_error(error: std::io::Error, _path: &Path) -> Self {
        let category = match error.kind() {
            std::io::ErrorKind::PermissionDenied => ErrorCategory::PermissionDenied,
            std::io::ErrorKind::NotFound => ErrorCategory::NotFound,
            _ => ErrorCategory::Unavailable,
        };
        Self {
            category,
            context: DiagnosticContext::new(DiagnosticCode::BrowserRead),
            source: Box::new(error),
        }
    }
}

impl fmt::Display for FolderReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "folder read error: {}", self.source)
    }
}

impl Error for FolderReadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

impl ErrorContract for FolderReadError {
    fn user_descriptor(&self) -> UserErrorDescriptor {
        UserErrorDescriptor::for_category(self.category)
    }

    fn diagnostic_context(&self) -> DiagnosticContext {
        self.context
    }
}

/// Port for reading the direct children of a filesystem folder.
///
/// Implementations enumerate a single folder level and return the results
/// sorted by [`BrowserEntry`] ordering (folders first, then files,
/// alphabetical within each group).
///
/// This trait is intentionally filesystem-agnostic — a fake implementation
/// can be used in tests without touching the real filesystem.
pub trait FolderReader: Send {
    /// Returns the direct children of the folder at `path`, sorted.
    ///
    /// Returns an error when the path does not exist, is not a directory,
    /// or the process lacks permission to read it.
    fn read_folder(&self, path: &Path) -> Result<Vec<BrowserEntry>, FolderReadError>;
}

#[cfg(test)]
mod tests {
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
}
