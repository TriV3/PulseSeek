use std::error::Error;
use std::fmt;
use std::path::Path;

use crate::error::{
    DiagnosticCode, DiagnosticContext, ErrorCategory, ErrorContract, UserErrorDescriptor,
};

/// Error returned by [`ExternalActions`] operations.
#[derive(Debug)]
pub struct ExternalActionError {
    category: ErrorCategory,
    context: DiagnosticContext,
    source: Box<dyn Error + Send + Sync + 'static>,
}

impl ExternalActionError {
    /// Builds an error from an underlying launch or filesystem error.
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

    /// Builds an `Unsupported` error for platforms without a reveal/open
    /// adapter.
    pub fn unsupported() -> Self {
        Self {
            category: ErrorCategory::Unsupported,
            context: DiagnosticContext::new(DiagnosticCode::FileOperation),
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "external file action unsupported on this platform",
            )),
        }
    }

    pub fn category(&self) -> ErrorCategory {
        self.category
    }
}

impl fmt::Display for ExternalActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "external file action error: {}", self.source)
    }
}

impl Error for ExternalActionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

impl ErrorContract for ExternalActionError {
    fn user_descriptor(&self) -> UserErrorDescriptor {
        UserErrorDescriptor::for_category(self.category)
    }

    fn diagnostic_context(&self) -> DiagnosticContext {
        self.context
    }
}

/// Port for revealing or opening a file through the operating system.
///
/// Implementations never expose a general process-launch capability to the UI;
/// callers receive only a typed result for a single, validated path.
pub trait ExternalActions: Send {
    /// Reveals `path` in the operating system file manager.
    fn reveal(&self, path: &Path) -> Result<(), ExternalActionError>;

    /// Opens `path` with the operating system's default application.
    fn open_with(&self, path: &Path) -> Result<(), ExternalActionError>;
}

#[cfg(test)]
mod tests;
