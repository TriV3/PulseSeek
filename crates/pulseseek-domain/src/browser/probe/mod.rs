use std::error::Error;
use std::fmt;
use std::path::Path;

use crate::error::{
    DiagnosticCode, DiagnosticContext, ErrorCategory, ErrorContract, UserErrorDescriptor,
};

/// Classification of a dropped filesystem path (FR-DI-001).
///
/// The webview delivers only path strings during an external drag-and-drop, so
/// the backend classifies each target before the UI decides whether to play it
/// or reveal its folder. `Missing` is a normal outcome, not an error: a file
/// removed between the drag start and the drop is simply ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeResult {
    /// The path is an existing directory.
    Directory,
    /// The path is an existing file the decoder can read.
    Playable,
    /// The path is an existing file with an unsupported format or a corrupt
    /// stream that cannot be decoded.
    Unsupported,
    /// The path does not exist.
    Missing,
}

/// Error returned by [`ProbeFile`] operations when the path cannot be
/// inspected (for example a permission denial while reading metadata).
#[derive(Debug)]
pub struct ProbeError {
    category: ErrorCategory,
    context: DiagnosticContext,
    source: Box<dyn Error + Send + Sync + 'static>,
}

impl ProbeError {
    /// Builds an error from an underlying filesystem error.
    ///
    /// `PermissionDenied` maps to its matching category; anything else maps to
    /// `Unavailable`. `NotFound` is deliberately not an error: a missing drop
    /// target is reported through [`ProbeResult::Missing`] instead. The raw
    /// path is never embedded in the user-facing message.
    pub fn from_io_error(error: std::io::Error, _path: &Path) -> Self {
        let category = match error.kind() {
            std::io::ErrorKind::PermissionDenied => ErrorCategory::PermissionDenied,
            _ => ErrorCategory::Unavailable,
        };
        Self {
            category,
            context: DiagnosticContext::new(DiagnosticCode::FileOperation),
            source: Box::new(error),
        }
    }

    pub fn category(&self) -> ErrorCategory {
        self.category
    }
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "path probe error: {}", self.source)
    }
}

impl Error for ProbeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

impl ErrorContract for ProbeError {
    fn user_descriptor(&self) -> UserErrorDescriptor {
        UserErrorDescriptor::for_category(self.category)
    }

    fn diagnostic_context(&self) -> DiagnosticContext {
        self.context
    }
}

/// Port for classifying a filesystem path delivered by an external
/// drag-and-drop (FR-DI-001).
///
/// Implementations inspect the path and return one of the [`ProbeResult`]
/// variants so the UI can decide between revealing a folder, playing an audio
/// file, or ignoring the target. Callers receive only a typed result; the UI
/// never receives a general filesystem or process-launch capability.
pub trait ProbeFile: Send + Sync {
    /// Classifies `path` for a drop target.
    ///
    /// Returns `Missing` when the path does not exist, `Directory` for a
    /// folder, `Playable` for a decodable audio file, and `Unsupported`
    /// otherwise. Inspection failures (for example a permission denial) are
    /// reported as a [`ProbeError`].
    fn probe(&self, path: &Path) -> Result<ProbeResult, ProbeError>;
}

#[cfg(test)]
mod tests;
