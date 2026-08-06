use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::error::{
    DiagnosticCode, DiagnosticContext, ErrorCategory, ErrorContract, UserErrorDescriptor,
};

/// Maximum length of a single file-name component.
///
/// 255 bytes is the per-component limit shared by macOS (APFS/HFS+), most
/// Linux filesystems, and NTFS. Names longer than this cannot be created on
/// any supported platform.
pub const MAX_FILE_NAME_BYTES: usize = 255;

/// Error returned by [`FileRename`] operations.
#[derive(Debug)]
pub struct RenameError {
    category: ErrorCategory,
    context: DiagnosticContext,
    source: Box<dyn Error + Send + Sync + 'static>,
}

impl RenameError {
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

    /// Builds an `InvalidInput` error for a name that fails validation.
    ///
    /// The reason is a static string that never embeds the raw path or name,
    /// so user-facing messages stay private.
    pub fn invalid_name(reason: &'static str) -> Self {
        Self {
            category: ErrorCategory::InvalidInput,
            context: DiagnosticContext::new(DiagnosticCode::FileOperation),
            source: Box::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, reason)),
        }
    }

    /// Builds a `Conflict` error for a target that already exists.
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

    pub fn category(&self) -> ErrorCategory {
        self.category
    }
}

impl fmt::Display for RenameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rename operation error: {}", self.source)
    }
}

impl Error for RenameError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

impl ErrorContract for RenameError {
    fn user_descriptor(&self) -> UserErrorDescriptor {
        UserErrorDescriptor::for_category(self.category)
    }

    fn diagnostic_context(&self) -> DiagnosticContext {
        self.context
    }
}

/// Validates a prospective file name for a rename.
///
/// The name must be a non-empty basename without path separators, without the
/// reserved `.` and `..` components, without a NUL byte, and within the
/// platform name-length limit. The raw name is never embedded in the returned
/// error message.
pub fn validate_rename_name(name: &str) -> Result<(), RenameError> {
    if name.is_empty() {
        return Err(RenameError::invalid_name("name is empty"));
    }
    if name == "." || name == ".." {
        return Err(RenameError::invalid_name("name is a reserved component"));
    }
    if name.contains('/') || name.contains('\\') || name.contains('\0') {
        return Err(RenameError::invalid_name("name contains a path separator or NUL"));
    }
    if name.len() > MAX_FILE_NAME_BYTES {
        return Err(RenameError::invalid_name("name is too long"));
    }
    Ok(())
}

/// Port for renaming a single file.
pub trait FileRename: Send {
    /// Renames `path` to `new_name` within the same directory and returns the
    /// resulting full path.
    ///
    /// The implementation validates the name and detects target collisions
    /// before touching the filesystem. Errors never embed the raw path in
    /// their user-facing message.
    fn rename(&self, path: &Path, new_name: &str) -> Result<PathBuf, RenameError>;
}

#[cfg(test)]
mod tests;
