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
        move_each_to_trash(paths, |path| {
            trash::delete(path).map_err(|err| map_trash_error(err, path))
        })
    }
}

fn move_each_to_trash(
    paths: &[PathBuf],
    mut move_one: impl FnMut(&Path) -> Result<(), TrashError>,
) -> Vec<TrashResult> {
    paths.iter().map(|path| (path.clone(), move_one(path))).collect()
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
mod tests;
