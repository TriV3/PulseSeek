use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use crate::error::{
    DiagnosticCode, DiagnosticContext, ErrorCategory, ErrorContract, UserErrorDescriptor,
};

/// Error returned by [`FileCopy`] operations.
#[derive(Debug)]
pub struct CopyError {
    category: ErrorCategory,
    context: DiagnosticContext,
    source: Box<dyn Error + Send + Sync + 'static>,
}

impl CopyError {
    pub fn from_io_error(error: std::io::Error, _path: &Path) -> Self {
        let category = match error.kind() {
            std::io::ErrorKind::PermissionDenied => ErrorCategory::PermissionDenied,
            std::io::ErrorKind::NotFound => ErrorCategory::NotFound,
            std::io::ErrorKind::AlreadyExists => ErrorCategory::Conflict,
            _ => ErrorCategory::Unavailable,
        };
        Self {
            category,
            context: DiagnosticContext::new(DiagnosticCode::FileOperation),
            source: Box::new(error),
        }
    }

    /// Builds an `InvalidInput` error for an invalid copy target.
    ///
    /// The reason is a static string that never embeds the raw path, so
    /// user-facing messages stay private.
    pub fn invalid_target(reason: &'static str) -> Self {
        Self {
            category: ErrorCategory::InvalidInput,
            context: DiagnosticContext::new(DiagnosticCode::FileOperation),
            source: Box::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, reason)),
        }
    }

    /// Builds a `Conflict` error for a target that already exists.
    ///
    /// PulseSeek never silently overwrites another file; the existing target
    /// is reported as a per-file failure and the rest of the batch continues.
    pub fn collision() -> Self {
        Self {
            category: ErrorCategory::Conflict,
            context: DiagnosticContext::new(DiagnosticCode::FileOperation),
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "target name already exists",
            )),
        }
    }

    /// Builds a `Cancelled` error reported for files that were not processed
    /// because the operation was cancelled.
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

    pub fn category(&self) -> ErrorCategory {
        self.category
    }
}

impl fmt::Display for CopyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "copy operation error: {}", self.source)
    }
}

impl Error for CopyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

impl ErrorContract for CopyError {
    fn user_descriptor(&self) -> UserErrorDescriptor {
        UserErrorDescriptor::for_category(self.category)
    }

    fn diagnostic_context(&self) -> DiagnosticContext {
        self.context
    }
}

/// One file's outcome: the source path plus either the new path after a
/// successful copy or a typed error. A single failing file never aborts the
/// rest of the batch, so partial failure is reported per target.
pub type CopyResult = (PathBuf, Result<PathBuf, CopyError>);

/// Port for copying files into a target directory.
pub trait FileCopy: Send {
    /// Copies every file in `files` into `target_dir`, keeping each file's
    /// name. `cancelled` is checked between files so a running copy can stop
    /// cooperatively; files that were not processed report a `Cancelled`
    /// error. Errors never embed the raw path in their user-facing message.
    fn copy_files(
        &self,
        files: &[PathBuf],
        target_dir: &Path,
        cancelled: &AtomicBool,
    ) -> Vec<CopyResult>;
}

#[cfg(test)]
mod tests;
