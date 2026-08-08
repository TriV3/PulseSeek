use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::error::{
    DiagnosticCode, DiagnosticContext, ErrorCategory, ErrorContract, UserErrorDescriptor,
};

/// Error returned by [`DragOut`] operations.
#[derive(Debug)]
pub struct DragOutError {
    category: ErrorCategory,
    context: DiagnosticContext,
    source: Box<dyn Error + Send + Sync + 'static>,
}

impl DragOutError {
    /// Builds an error from an underlying filesystem error.
    ///
    /// `NotFound` and `PermissionDenied` map to their matching categories;
    /// anything else maps to `Unavailable`. The raw path is never embedded in
    /// the user-facing message.
    pub fn from_io_error(error: std::io::Error, _path: &Path) -> Self {
        let category = match error.kind() {
            std::io::ErrorKind::NotFound => ErrorCategory::NotFound,
            std::io::ErrorKind::PermissionDenied => ErrorCategory::PermissionDenied,
            _ => ErrorCategory::Unavailable,
        };
        Self {
            category,
            context: DiagnosticContext::new(DiagnosticCode::FileOperation),
            source: Box::new(error),
        }
    }

    /// Builds an `InvalidInput` error for an empty selection.
    pub fn empty_selection() -> Self {
        Self {
            category: ErrorCategory::InvalidInput,
            context: DiagnosticContext::new(DiagnosticCode::FileOperation),
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "no files selected to drag",
            )),
        }
    }

    /// Builds a `Cancelled` error when the user aborts the drag.
    pub fn cancelled() -> Self {
        Self {
            category: ErrorCategory::Cancelled,
            context: DiagnosticContext::new(DiagnosticCode::FileOperation),
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "drag cancelled",
            )),
        }
    }

    /// Builds an `Unsupported` error for platforms without a drag adapter.
    pub fn unsupported() -> Self {
        Self {
            category: ErrorCategory::Unsupported,
            context: DiagnosticContext::new(DiagnosticCode::FileOperation),
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "file drag-out unsupported on this platform",
            )),
        }
    }

    pub fn category(&self) -> ErrorCategory {
        self.category
    }
}

impl fmt::Display for DragOutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "file drag-out error: {}", self.source)
    }
}

impl Error for DragOutError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

impl ErrorContract for DragOutError {
    fn user_descriptor(&self) -> UserErrorDescriptor {
        UserErrorDescriptor::for_category(self.category)
    }

    fn diagnostic_context(&self) -> DiagnosticContext {
        self.context
    }
}

/// Port for dragging files out of the application into compatible external
/// applications (FR-FM-011).
///
/// Implementations validate the requested paths and then initiate the
/// operating system drag session. Callers receive only a typed result; the UI
/// never receives a general process-launch or clipboard capability.
pub trait DragOut: Send + Sync {
    /// Starts a drag session for `paths`.
    ///
    /// Returns `Cancelled` when the user aborts the drag, `NotFound` when any
    /// target is missing, and `Unsupported` when the platform has no adapter.
    fn drag_out(&self, paths: &[PathBuf]) -> Result<(), DragOutError>;
}

#[cfg(test)]
mod tests;
