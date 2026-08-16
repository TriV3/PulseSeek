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

pub type TrashResult = (PathBuf, Result<(), TrashError>);

/// Port for moving files to the operating system trash.
pub trait FileTrash: Send {
    fn move_to_trash(&self, paths: &[PathBuf]) -> Vec<TrashResult>;
}

#[cfg(test)]
#[allow(clippy::type_complexity)]
mod tests;
