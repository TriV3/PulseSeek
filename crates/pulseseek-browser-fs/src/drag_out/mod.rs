use std::path::PathBuf;

use pulseseek_domain::browser::drag_out::{DragOut, DragOutError};

/// Platform-specific drag session starter.
///
/// Implementations initiate the operating system drag session for the given
/// paths. The filesystem adapter validates the paths before delegating here,
/// so a starter never receives a missing target.
pub trait DragStarter: Send + Sync {
    /// Starts a drag session for `paths`, returning `Cancelled` when the user
    /// aborts the drag.
    fn start(&self, paths: &[PathBuf]) -> Result<(), DragOutError>;
}

/// Native filesystem adapter for [`DragOut`].
///
/// Validates that every requested path exists before delegating to the
/// platform [`DragStarter`], so a missing target is reported as `NotFound`
/// without ever initiating a drag session.
pub struct NativeDragOut<S: DragStarter> {
    starter: S,
}

impl<S: DragStarter> NativeDragOut<S> {
    pub fn new(starter: S) -> Self {
        Self { starter }
    }
}

impl<S: DragStarter> DragOut for NativeDragOut<S> {
    fn drag_out(&self, paths: &[PathBuf]) -> Result<(), DragOutError> {
        if paths.is_empty() {
            return Err(DragOutError::empty_selection());
        }
        for path in paths {
            if !path.exists() {
                return Err(DragOutError::from_io_error(
                    std::io::Error::new(std::io::ErrorKind::NotFound, "file does not exist"),
                    path,
                ));
            }
        }
        self.starter.start(paths)
    }
}

#[cfg(test)]
mod tests;
