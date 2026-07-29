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

    // Probe readability by listing metadata.
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
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn valid_directory_returns_ok() {
        let dir = tempdir().expect("tempdir creation");
        assert!(validate_directory(dir.path().to_str().unwrap()).is_ok());
    }

    #[test]
    fn nonexistent_path_fails() {
        let result = validate_directory("/tmp/pulseseek-nonexistent-xxxxxxxx");
        assert!(result.is_err());
    }

    #[test]
    fn file_path_fails() {
        let dir = tempdir().expect("tempdir creation");
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "content").expect("write test file");
        let result = validate_directory(file_path.to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    #[cfg(unix)]
    fn unreadable_directory_fails() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().expect("tempdir creation");
        let mut perms = fs::metadata(dir.path()).unwrap().permissions();
        perms.set_mode(0o000);
        fs::set_permissions(dir.path(), perms).expect("set permissions");

        // Skip the test when permissions cannot be removed (e.g. running as
        // root in a container).
        if fs::read_dir(dir.path()).is_ok() {
            // Restore permissions so the tempdir can be cleaned up.
            let mut perms = fs::metadata(dir.path()).unwrap().permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(dir.path(), perms);
            return;
        }

        let result = validate_directory(dir.path().to_str().unwrap());
        assert!(result.is_err());

        // Restore permissions so the tempdir can be cleaned up.
        let mut perms = fs::metadata(dir.path()).unwrap().permissions();
        perms.set_mode(0o755);
        let _ = fs::set_permissions(dir.path(), perms);
    }
}
