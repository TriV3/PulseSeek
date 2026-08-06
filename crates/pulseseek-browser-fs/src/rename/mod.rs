use std::path::{Path, PathBuf};

use pulseseek_domain::browser::rename::{validate_rename_name, FileRename, RenameError};

/// Native filesystem adapter for [`FileRename`].
///
/// Renames a file within its current directory using `std::fs::rename`. The
/// name is validated and target collisions are detected before touching the
/// filesystem, so a rename never silently overwrites another file. On POSIX
/// the underlying rename may still replace a target that appears after the
/// existence check (a narrow TOCTOU window); that race is left to the
/// operating system and is documented in the file-rename architecture note.
pub struct NativeFileRename;

impl FileRename for NativeFileRename {
    fn rename(&self, path: &Path, new_name: &str) -> Result<PathBuf, RenameError> {
        validate_rename_name(new_name)?;
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or_else(|| RenameError::invalid_name("path has no parent directory"))?;
        let target = parent.join(new_name);

        if target == path {
            // Renaming to the current name is a successful no-op.
            return Ok(target);
        }
        if target.exists() {
            return Err(RenameError::collision());
        }

        std::fs::rename(path, &target).map_err(|error| RenameError::from_io_error(error, path))?;
        Ok(target)
    }
}

#[cfg(test)]
mod tests;
