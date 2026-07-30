use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::error::{
    DiagnosticCode, DiagnosticContext, ErrorCategory, ErrorContract, UserErrorDescriptor,
};

/// Error returned by [`FileTrash`] operations.
#[derive(Debug)]
pub struct TrashError {
    category: ErrorCategory,
    context: DiagnosticContext,
    source: Box<dyn Error + Send + Sync + 'static>,
}

impl TrashError {
    /// Creates a new error from an `std::io::Error` and the path being moved.
    pub fn from_io_error(error: std::io::Error, _path: &Path) -> Self {
        let category = match error.kind() {
            std::io::ErrorKind::PermissionDenied => ErrorCategory::PermissionDenied,
            std::io::ErrorKind::NotFound => ErrorCategory::NotFound,
            _ => ErrorCategory::Unavailable,
        };
        Self {
            category,
            context: DiagnosticContext::new(DiagnosticCode::FileOperation),
            source: Box::new(error),
        }
    }

    /// Creates a new error from a cancellation source.
    pub fn cancelled() -> Self {
        Self {
            category: ErrorCategory::Cancelled,
            context: DiagnosticContext::new(DiagnosticCode::FileOperation),
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "operation cancelled",
            )),
        }
    }

    /// Returns the error category.
    pub fn category(&self) -> ErrorCategory {
        self.category
    }
}

impl fmt::Display for TrashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "trash operation error: {}", self.source)
    }
}

impl Error for TrashError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

impl ErrorContract for TrashError {
    fn user_descriptor(&self) -> UserErrorDescriptor {
        UserErrorDescriptor::for_category(self.category)
    }

    fn diagnostic_context(&self) -> DiagnosticContext {
        self.context
    }
}

/// Result of moving a single file to the trash.
pub type TrashResult = (PathBuf, Result<(), TrashError>);

/// Port for moving files to the operating system trash.
///
/// Implementations delegate to the platform-native trash (Recycle Bin on
/// Windows, Trash on macOS, `$XDG_DATA_HOME/Trash` on Linux).
///
/// This trait is intentionally filesystem-agnostic — a fake implementation
/// can be used in tests without touching the real filesystem.
pub trait FileTrash: Send {
    /// Moves every file in `paths` to the trash.
    ///
    /// Returns a `Vec<TrashResult>` with one entry per input path in the
    /// same order. Successful moves yield `Ok(())`; failed moves yield the
    /// corresponding `TrashError`.
    ///
    /// Callers must never interpret a missing return entry as success.
    /// Every input path has exactly one output entry.
    fn move_to_trash(&self, paths: &[PathBuf]) -> Vec<TrashResult>;
}

#[cfg(test)]
#[allow(clippy::type_complexity)]
mod tests {
    use super::*;

    // ── TrashError ErrorContract tests ────────────────────────────────

    #[test]
    fn trash_error_implements_error_contract() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "test");
        let err = TrashError::from_io_error(io_err, Path::new("/test"));
        let desc = err.user_descriptor();
        assert!(!desc.message().is_empty(), "error should have a safe message");
        assert_eq!(err.diagnostic_context().code(), "file.operation");
    }

    #[test]
    fn trash_error_permission_denied_category() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let err = TrashError::from_io_error(io_err, Path::new("/test"));
        assert_eq!(err.user_descriptor().category(), ErrorCategory::PermissionDenied);
    }

    #[test]
    fn trash_error_not_found_category() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let err = TrashError::from_io_error(io_err, Path::new("/test"));
        assert_eq!(err.user_descriptor().category(), ErrorCategory::NotFound);
    }

    #[test]
    fn trash_error_other_errors_are_unavailable() {
        let io_err = std::io::Error::other("other");
        let err = TrashError::from_io_error(io_err, Path::new("/test"));
        assert_eq!(err.user_descriptor().category(), ErrorCategory::Unavailable);
    }

    #[test]
    fn trash_error_cancelled_has_cancelled_category() {
        let err = TrashError::cancelled();
        assert_eq!(err.user_descriptor().category(), ErrorCategory::Cancelled);
    }

    // ── FileTrash trait contract tests ─────────────────────────────────

    /// Fake implementation that delegates to a closure for result generation.
    struct FakeFileTrash {
        f: Box<dyn Fn(&[PathBuf]) -> Vec<TrashResult> + Send>,
    }

    impl FakeFileTrash {
        fn new(f: Box<dyn Fn(&[PathBuf]) -> Vec<TrashResult> + Send>) -> Self {
            Self { f }
        }
    }

    impl FileTrash for FakeFileTrash {
        fn move_to_trash(&self, paths: &[PathBuf]) -> Vec<TrashResult> {
            (self.f)(paths)
        }
    }

    #[test]
    fn file_trash_returns_one_result_per_input_path() {
        let paths = vec![PathBuf::from("/music/a.wav"), PathBuf::from("/music/b.wav")];
        let paths_copy = paths.clone();
        let trash = FakeFileTrash::new(Box::new(move |_| {
            paths_copy.iter().map(|p| (p.clone(), Ok(()))).collect()
        }));
        let results = trash.move_to_trash(&paths);
        assert_eq!(results.len(), 2, "should return one result per input path");
        assert!(results.iter().all(|(_, r)| r.is_ok()), "all should succeed");
    }

    #[test]
    fn file_trash_reports_partial_batch_failure() {
        let paths = vec![
            PathBuf::from("/music/good.wav"),
            PathBuf::from("/music/bad.wav"),
            PathBuf::from("/music/also_good.wav"),
        ];
        let trash = FakeFileTrash::new(Box::new(|_| {
            vec![
                (PathBuf::from("/music/good.wav"), Ok(())),
                (
                    PathBuf::from("/music/bad.wav"),
                    Err(TrashError::from_io_error(
                        std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
                        Path::new("/music/bad.wav"),
                    )),
                ),
                (PathBuf::from("/music/also_good.wav"), Ok(())),
            ]
        }));
        let results = trash.move_to_trash(&paths);
        assert_eq!(results.len(), 3);
        assert!(results[0].1.is_ok());
        assert!(results[1].1.is_err());
        assert_eq!(
            results[1].1.as_ref().unwrap_err().user_descriptor().category(),
            ErrorCategory::NotFound
        );
        assert!(results[2].1.is_ok());
    }

    #[test]
    fn file_trash_accepts_empty_list() {
        let trash = FakeFileTrash::new(Box::new(|_| vec![]));
        let results = trash.move_to_trash(&[]);
        assert!(results.is_empty(), "empty input yields empty results");
    }
}
