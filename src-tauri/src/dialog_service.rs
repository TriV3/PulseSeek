use pulseseek_domain::error::{ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory};

use crate::path_validation;

/// Abstraction over the native OS folder-picker dialog.
///
/// Enables unit-testing code paths that require a folder selection without
/// spawning a real system dialog.
pub trait FolderPicker: Send + Sync {
    /// Opens the native folder picker dialog.
    ///
    /// Returns `Ok(Some(path))` when the user selects a folder, `Ok(None)`
    /// when the user cancels, or `Err(ApplicationError)` when the dialog
    /// encounters a system-level failure.
    fn pick_folder(&self) -> Result<Option<String>, ApplicationError>;
}

// ── Real implementation (Tauri) ─────────────────────────────────────────

/// Tauri-backed folder picker that delegates to the native OS dialog via
/// `tauri-plugin-dialog`.
pub struct TauriFolderPicker {
    app: tauri::AppHandle,
}

impl TauriFolderPicker {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

impl FolderPicker for TauriFolderPicker {
    fn pick_folder(&self) -> Result<Option<String>, ApplicationError> {
        use tauri_plugin_dialog::DialogExt;

        let picked = self.app.dialog().file().blocking_pick_folder();

        match picked {
            Some(file_path) => {
                let path_str = file_path.to_string();
                path_validation::validate_directory(&path_str)?;
                Ok(Some(path_str))
            },
            None => Ok(None),
        }
    }
}

// ── Fake implementation (tests) ─────────────────────────────────────────

/// Fake folder picker that returns a pre-configured result without showing
/// a system dialog. Used in unit tests.
pub struct FakeFolderPicker {
    inner: Box<dyn Fn() -> Result<Option<String>, ApplicationError> + Send + Sync>,
}

impl FakeFolderPicker {
    /// Creates a picker that always returns the given path (or `None` for
    /// cancellation) without errors.
    pub fn returning(path: Option<&str>) -> Self {
        let owned = path.map(|p| p.to_string());
        Self { inner: Box::new(move || Ok(owned.clone())) }
    }

    /// Creates a picker that fails with a permission-denied error.
    pub fn failing() -> Self {
        Self {
            inner: Box::new(|| {
                Err(ApplicationError::new(
                    ErrorCategory::PermissionDenied,
                    DiagnosticContext::new(DiagnosticCode::BrowserRead),
                    std::io::Error::new(std::io::ErrorKind::PermissionDenied, "simulated"),
                ))
            }),
        }
    }
}

impl FolderPicker for FakeFolderPicker {
    fn pick_folder(&self) -> Result<Option<String>, ApplicationError> {
        (self.inner)()
    }
}
