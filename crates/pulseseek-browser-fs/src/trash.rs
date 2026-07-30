use std::path::{Path, PathBuf};

use pulseseek_domain::browser::trash::{FileTrash, TrashError, TrashResult};

/// Native filesystem adapter for [`FileTrash`].
///
/// Delegates to the operating system's native trash via the `trash` crate.
/// Each file is moved individually so that a single failure does not abort
/// the entire batch.
pub struct NativeFileTrash;

impl FileTrash for NativeFileTrash {
    fn move_to_trash(&self, paths: &[PathBuf]) -> Vec<TrashResult> {
        paths
            .iter()
            .map(|path| match trash::delete(path) {
                Ok(()) => (path.clone(), Ok(())),
                Err(err) => (path.clone(), Err(map_trash_error(err, path))),
            })
            .collect()
    }
}

fn map_trash_error(err: trash::Error, path: &Path) -> TrashError {
    match &err {
        trash::Error::TargetedRoot
        | trash::Error::CouldNotAccess { .. }
        | trash::Error::CanonicalizePath { .. }
        | trash::Error::ConvertOsString { .. } => TrashError::from_io_error(
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, err.to_string()),
            path,
        ),
        _ => TrashError::from_io_error(std::io::Error::other(err.to_string()), path),
    }
}

#[cfg(test)]
#[allow(clippy::type_complexity)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn native_trash_moves_file() {
        let dir = tempdir().expect("tempdir");
        let file_path = dir.path().join("to_trash.txt");
        std::fs::write(&file_path, "content").expect("write test file");
        assert!(file_path.exists(), "file should exist before trash");

        let results = NativeFileTrash.move_to_trash(std::slice::from_ref(&file_path));
        assert_eq!(results.len(), 1);
        assert!(results[0].1.is_ok(), "file should be trashed successfully: {:?}", results[0].1);
        // File should no longer exist at original path.
        assert!(!file_path.exists(), "file should be removed after trash");
    }

    #[test]
    fn native_trash_nonexistent_file_returns_error() {
        let missing = PathBuf::from("/tmp/pulseseek-nonexistent-xxxxxxxx.wav");
        let results = NativeFileTrash.move_to_trash(&[missing]);
        assert_eq!(results.len(), 1);
        assert!(results[0].1.is_err(), "trashing nonexistent file should produce an error");
    }

    #[test]
    fn native_trash_reports_partial_batch_failure() {
        let dir = tempdir().expect("tempdir");
        let good = dir.path().join("good.wav");
        let missing = PathBuf::from("/tmp/pulseseek-nonexistent-yyyyyyyy.wav");
        std::fs::write(&good, "content").expect("write test file");

        let results = NativeFileTrash.move_to_trash(&[good.clone(), missing]);
        assert_eq!(results.len(), 2);
        // First: good file should succeed.
        assert!(results[0].1.is_ok(), "good file should be trashed");
        // Second: missing file should fail.
        assert!(results[1].1.is_err(), "missing file should error");
    }

    #[test]
    fn native_trash_empty_list_returns_empty() {
        let results = NativeFileTrash.move_to_trash(&[]);
        assert!(results.is_empty());
    }
}
