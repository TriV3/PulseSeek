use std::path::Path;
use std::process::{Command, Stdio};

use pulseseek_domain::browser::external_actions::{ExternalActionError, ExternalActions};

/// Native filesystem adapter for [`ExternalActions`].
///
/// `open_with` delegates to the `open` crate, which launches the operating
/// system's default application for the path. `reveal` selects the file in the
/// operating system file manager using a small platform-specific command:
/// `open -R` on macOS, `explorer /select` on Windows, and opening the parent
/// directory via `xdg-open` on other Unix-like systems.
///
/// Both operations validate that the path exists first, so a missing file is
/// reported as `NotFound` without ever launching a process.
pub struct NativeFileActions;

impl ExternalActions for NativeFileActions {
    fn reveal(&self, path: &Path) -> Result<(), ExternalActionError> {
        ensure_exists(path)?;
        reveal_in_file_manager(path)
    }

    fn open_with(&self, path: &Path) -> Result<(), ExternalActionError> {
        ensure_exists(path)?;
        open::that(path).map_err(|error| ExternalActionError::from_io_error(error, path))
    }
}

fn ensure_exists(path: &Path) -> Result<(), ExternalActionError> {
    if path.exists() {
        Ok(())
    } else {
        Err(ExternalActionError::from_io_error(
            std::io::Error::new(std::io::ErrorKind::NotFound, "file does not exist"),
            path,
        ))
    }
}

#[cfg(target_os = "macos")]
fn reveal_in_file_manager(path: &Path) -> Result<(), ExternalActionError> {
    run_reveal_command("open", &["-R", &path.to_string_lossy()])
}

#[cfg(target_os = "windows")]
fn reveal_in_file_manager(path: &Path) -> Result<(), ExternalActionError> {
    let select = format!("/select,{}", path.to_string_lossy());
    run_reveal_command("explorer", &[&select])
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn reveal_in_file_manager(path: &Path) -> Result<(), ExternalActionError> {
    // Fallback: reveal the containing directory with the platform file manager.
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    run_reveal_command("xdg-open", &[&parent.to_string_lossy()])
}

fn run_reveal_command(program: &str, args: &[&str]) -> Result<(), ExternalActionError> {
    let status = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| ExternalActionError::from_io_error(error, Path::new(program)))?;
    if status.success() {
        Ok(())
    } else {
        Err(ExternalActionError::from_io_error(
            std::io::Error::other(format!("reveal launcher exited with {status:?}")),
            Path::new(program),
        ))
    }
}

#[cfg(test)]
mod tests;
