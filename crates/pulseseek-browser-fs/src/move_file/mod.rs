use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use pulseseek_domain::browser::move_file::{FileMove, MoveError, MoveResult};

/// Native filesystem adapter for [`FileMove`].
///
/// Moves each file into `target_dir` with `std::fs::rename` and keeps each
/// file's name. Target collisions are detected before touching the
/// filesystem, so a move never silently overwrites another file. When the
/// rename fails with `CrossesDevices` — the source and target live on
/// different volumes and no platform offers an atomic cross-volume move —
/// the adapter falls back to copy-then-delete through [`copy_then_remove`].
/// On POSIX the underlying rename may still replace a target that appears
/// after the existence check (a narrow TOCTOU window); that race is left to
/// the operating system and documented in the file-move architecture note.
type RenameFn = Box<dyn Fn(&Path, &Path) -> std::io::Result<()> + Send + Sync>;

pub struct NativeFileMover {
    rename: RenameFn,
}

impl NativeFileMover {
    pub fn new() -> Self {
        Self { rename: Box::new(|source, target| std::fs::rename(source, target)) }
    }

    /// Builds a mover whose rename step is replaced, used by tests to force
    /// cross-device behavior without a second volume.
    pub fn with_rename(f: RenameFn) -> Self {
        Self { rename: f }
    }

    fn move_one(&self, path: &Path, target_dir: &Path) -> Result<PathBuf, MoveError> {
        if path.is_dir() {
            return Err(MoveError::invalid_target("moving directories is not supported"));
        }
        if !path.exists() {
            return Err(MoveError::from_io_error(
                std::io::Error::new(std::io::ErrorKind::NotFound, "source file is missing"),
                path,
            ));
        }
        if !target_dir.is_dir() {
            return Err(MoveError::invalid_target("target directory is not a directory"));
        }
        let name =
            path.file_name().ok_or_else(|| MoveError::invalid_target("path has no file name"))?;
        let target = target_dir.join(name);

        if target == path {
            // Moving a file into its own directory is a successful no-op.
            return Ok(target);
        }
        if target.exists() {
            return Err(MoveError::collision());
        }

        match (self.rename)(path, &target) {
            Ok(()) => Ok(target),
            Err(error) if error.kind() == std::io::ErrorKind::CrossesDevices => {
                copy_then_remove(path, &target)
            },
            Err(error) => Err(MoveError::from_io_error(error, path)),
        }
    }
}

impl Default for NativeFileMover {
    fn default() -> Self {
        Self::new()
    }
}

impl FileMove for NativeFileMover {
    fn move_files(
        &self,
        files: &[PathBuf],
        target_dir: &Path,
        cancelled: &AtomicBool,
    ) -> Vec<MoveResult> {
        files
            .iter()
            .map(|path| {
                if cancelled.load(Ordering::Acquire) {
                    return (path.clone(), Err(MoveError::cancelled()));
                }
                let result = self.move_one(path, target_dir);
                (path.clone(), result)
            })
            .collect()
    }
}

/// Cross-volume fallback for [`NativeFileMover`]: copy `source` to `target`
/// and only then remove the source, so a failed copy never loses the
/// original. A failed copy removes the partial target again (best effort)
/// before reporting, and a failed source removal removes the copy again so
/// the move rolls back instead of leaving a duplicate.
pub fn copy_then_remove(source: &Path, target: &Path) -> Result<PathBuf, MoveError> {
    if let Err(error) = std::fs::copy(source, target) {
        let _ = std::fs::remove_file(target);
        return Err(MoveError::from_io_error(error, source));
    }
    if let Err(error) = std::fs::remove_file(source) {
        let _ = std::fs::remove_file(target);
        return Err(MoveError::from_io_error(error, source));
    }
    Ok(target.to_path_buf())
}

#[cfg(test)]
mod tests;
