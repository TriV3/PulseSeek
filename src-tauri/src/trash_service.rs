use std::path::PathBuf;

use pulseseek_domain::browser::trash::{FileTrash, TrashResult};
use pulseseek_domain::error::{ApplicationError, ErrorContract};

/// Application service for moving files to the operating system trash.
///
/// This trait abstracts the [`FileTrash`] domain port behind a narrow
/// interface that returns application-level errors suitable for the Tauri
/// boundary. Batch operations return per-item results.
pub trait TrashService: Send {
    /// Moves every file in `paths` to the trash.
    ///
    /// Returns a vector of `(PathBuf, Result<(), ApplicationError>)` in path
    /// order. Successful trashes yield `Ok(())`; failures yield a typed
    /// error mapped from [`TrashError`].
    fn move_to_trash(&self, paths: Vec<PathBuf>) -> Vec<(PathBuf, Result<(), ApplicationError>)>;
}

// ── Native implementation ──────────────────────────────────────────────

/// Wraps a [`FileTrash`] adapter and converts domain errors into
/// application errors.
#[allow(clippy::type_complexity)]
pub struct NativeTrashService<T: FileTrash> {
    inner: T,
}

impl<T: FileTrash> NativeTrashService<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
}

impl<T: FileTrash> TrashService for NativeTrashService<T> {
    fn move_to_trash(&self, paths: Vec<PathBuf>) -> Vec<(PathBuf, Result<(), ApplicationError>)> {
        let results: Vec<TrashResult> = self.inner.move_to_trash(&paths);
        results
            .into_iter()
            .map(|(path, result)| {
                let mapped = result.map_err(|e| {
                    let category = e.category();
                    let context = e.diagnostic_context();
                    ApplicationError::new(category, context, e)
                });
                (path, mapped)
            })
            .collect()
    }
}

// ── Fake implementation (tests) ────────────────────────────────────────

/// Fake trash service that returns pre-configured results without touching
/// the filesystem. Uses a closure so callers can inject arbitrary results
/// without requiring `ApplicationError` to be `Clone`.
#[allow(clippy::type_complexity)]
pub struct FakeTrashService {
    f: Box<dyn Fn(Vec<PathBuf>) -> Vec<(PathBuf, Result<(), ApplicationError>)> + Send>,
}

#[allow(clippy::type_complexity)]
impl FakeTrashService {
    pub fn new(
        f: Box<dyn Fn(Vec<PathBuf>) -> Vec<(PathBuf, Result<(), ApplicationError>)> + Send>,
    ) -> Self {
        Self { f }
    }
}

impl TrashService for FakeTrashService {
    fn move_to_trash(&self, paths: Vec<PathBuf>) -> Vec<(PathBuf, Result<(), ApplicationError>)> {
        (self.f)(paths)
    }
}

#[cfg(test)]
mod tests {
    use pulseseek_domain::error::{
        DiagnosticCode, DiagnosticContext, ErrorCategory, ErrorContract,
    };

    use super::*;

    fn make_app_error(category: ErrorCategory) -> ApplicationError {
        ApplicationError::new(
            category,
            DiagnosticContext::new(DiagnosticCode::FileOperation),
            std::io::Error::other("fake error"),
        )
    }

    #[test]
    fn trash_service_returns_one_result_per_path() {
        let paths = vec![PathBuf::from("/music/a.wav"), PathBuf::from("/music/b.wav")];
        let service = FakeTrashService::new(Box::new(|_| {
            vec![(PathBuf::from("/music/a.wav"), Ok(())), (PathBuf::from("/music/b.wav"), Ok(()))]
        }));
        let output = service.move_to_trash(paths);
        assert_eq!(output.len(), 2);
        assert!(output[0].1.is_ok());
        assert!(output[1].1.is_ok());
    }

    #[test]
    fn trash_service_reports_permission_failure() {
        let paths = vec![PathBuf::from("/music/secret.wav")];
        let service = FakeTrashService::new(Box::new(|_| {
            let err = make_app_error(ErrorCategory::PermissionDenied);
            vec![(PathBuf::from("/music/secret.wav"), Err(err))]
        }));
        let output = service.move_to_trash(paths);
        assert_eq!(output.len(), 1);
        assert!(output[0].1.is_err());
        assert_eq!(
            output[0].1.as_ref().unwrap_err().user_descriptor().category(),
            ErrorCategory::PermissionDenied
        );
    }

    #[test]
    fn trash_service_reports_cancellation() {
        let paths = vec![PathBuf::from("/music/a.wav")];
        let service = FakeTrashService::new(Box::new(|_| {
            let err = make_app_error(ErrorCategory::Cancelled);
            vec![(PathBuf::from("/music/a.wav"), Err(err))]
        }));
        let output = service.move_to_trash(paths);
        assert_eq!(output.len(), 1);
        assert!(output[0].1.is_err());
        assert_eq!(
            output[0].1.as_ref().unwrap_err().user_descriptor().category(),
            ErrorCategory::Cancelled
        );
    }

    #[test]
    fn trash_service_reports_partial_batch_failure() {
        let paths = vec![PathBuf::from("/music/good.wav"), PathBuf::from("/music/bad.wav")];
        let service = FakeTrashService::new(Box::new(|_| {
            let err = make_app_error(ErrorCategory::NotFound);
            vec![
                (PathBuf::from("/music/good.wav"), Ok(())),
                (PathBuf::from("/music/bad.wav"), Err(err)),
            ]
        }));
        let output = service.move_to_trash(paths);
        assert_eq!(output.len(), 2);
        assert!(output[0].1.is_ok());
        assert!(output[1].1.is_err());
        assert_eq!(
            output[1].1.as_ref().unwrap_err().user_descriptor().category(),
            ErrorCategory::NotFound
        );
    }

    #[test]
    fn trash_service_accepts_empty_list() {
        let service = FakeTrashService::new(Box::new(|_| vec![]));
        let output = service.move_to_trash(vec![]);
        assert!(output.is_empty());
    }
}
