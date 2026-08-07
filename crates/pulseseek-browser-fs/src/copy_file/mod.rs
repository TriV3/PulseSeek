use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use pulseseek_domain::browser::copy_file::{CopyError, CopyResult, FileCopy};

/// Native filesystem adapter for [`FileCopy`].
///
/// Copies each file into `target_dir` with `std::fs::copy` and keeps each
/// file's name. Target collisions are detected before touching the
/// filesystem, so a copy never silently overwrites another file: the
/// existing target is reported as a per-file `Conflict` and the batch
/// continues (PR-077 collision policy). A failed copy removes the partial
/// target again (best effort) so no orphaned half-copied file remains.
/// Originals are never modified: copying is read-only over the source.
pub struct NativeFileCopier;

impl NativeFileCopier {
    pub fn new() -> Self {
        Self
    }

    fn copy_one(&self, path: &Path, target_dir: &Path) -> Result<PathBuf, CopyError> {
        if path.is_dir() {
            return Err(CopyError::invalid_target("copying directories is not supported"));
        }
        if !path.exists() {
            return Err(CopyError::from_io_error(
                std::io::Error::new(std::io::ErrorKind::NotFound, "source file is missing"),
                path,
            ));
        }
        if !target_dir.is_dir() {
            return Err(CopyError::invalid_target("target directory is not a directory"));
        }
        let name =
            path.file_name().ok_or_else(|| CopyError::invalid_target("path has no file name"))?;
        let target = target_dir.join(name);

        if target == path {
            // Copying a file into its own directory would overwrite the
            // source; report it as a conflict so the source stays intact.
            return Err(CopyError::collision());
        }
        if target.exists() {
            return Err(CopyError::collision());
        }

        match std::fs::copy(path, &target) {
            Ok(_) => Ok(target),
            Err(error) => {
                let _ = std::fs::remove_file(&target);
                Err(CopyError::from_io_error(error, path))
            },
        }
    }
}

impl Default for NativeFileCopier {
    fn default() -> Self {
        Self::new()
    }
}

impl FileCopy for NativeFileCopier {
    fn copy_files(
        &self,
        files: &[PathBuf],
        target_dir: &Path,
        cancelled: &AtomicBool,
    ) -> Vec<CopyResult> {
        files
            .iter()
            .map(|path| {
                if cancelled.load(Ordering::Acquire) {
                    return (path.clone(), Err(CopyError::cancelled()));
                }
                let result = self.copy_one(path, target_dir);
                (path.clone(), result)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
