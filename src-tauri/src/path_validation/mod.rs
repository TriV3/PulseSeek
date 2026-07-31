use std::path::Path;

use pulseseek_domain::error::{ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory};

/// Validates that `path_str` exists, is a directory, and is readable.
///
/// Returns `Ok(())` when the path is valid. Returns an `ApplicationError`
/// with a safe user message and diagnostic code on failure.
pub fn validate_directory(path_str: &str) -> Result<(), ApplicationError> {
    let path = Path::new(path_str);

    if !path.exists() {
        return Err(ApplicationError::new(
            ErrorCategory::InvalidInput,
            DiagnosticContext::new(DiagnosticCode::BrowserRead),
            std::io::Error::new(std::io::ErrorKind::NotFound, "path does not exist"),
        ));
    }

    if !path.is_dir() {
        return Err(ApplicationError::new(
            ErrorCategory::InvalidInput,
            DiagnosticContext::new(DiagnosticCode::BrowserRead),
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "path is not a directory"),
        ));
    }

    match path.read_dir() {
        Ok(_) => Ok(()),
        Err(e) => Err(ApplicationError::new(
            ErrorCategory::PermissionDenied,
            DiagnosticContext::new(DiagnosticCode::BrowserRead),
            e,
        )),
    }
}

#[cfg(test)]
mod tests;
