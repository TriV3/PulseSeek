use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use super::*;

#[allow(clippy::type_complexity)]
struct FakeFileMove {
    f: Box<dyn Fn(&[PathBuf], &Path, &AtomicBool) -> Vec<MoveResult> + Send>,
}

impl FakeFileMove {
    #[allow(clippy::type_complexity)]
    fn new(f: Box<dyn Fn(&[PathBuf], &Path, &AtomicBool) -> Vec<MoveResult> + Send>) -> Self {
        Self { f }
    }
}

impl FileMove for FakeFileMove {
    fn move_files(
        &self,
        files: &[PathBuf],
        target_dir: &Path,
        cancelled: &AtomicBool,
    ) -> Vec<MoveResult> {
        (self.f)(files, target_dir, cancelled)
    }
}

#[test]
fn move_error_implements_error_contract() {
    let err = MoveError::from_io_error(
        std::io::Error::new(std::io::ErrorKind::PermissionDenied, "test"),
        Path::new("/test"),
    );
    let desc = err.user_descriptor();
    assert!(!desc.message().is_empty(), "error should have a safe message");
    assert_eq!(err.diagnostic_context().code(), "file.operation");
}

#[test]
fn move_error_permission_denied_category() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
    let err = MoveError::from_io_error(io_err, Path::new("/test"));
    assert_eq!(err.user_descriptor().category(), ErrorCategory::PermissionDenied);
}

#[test]
fn move_error_not_found_category() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
    let err = MoveError::from_io_error(io_err, Path::new("/test"));
    assert_eq!(err.user_descriptor().category(), ErrorCategory::NotFound);
}

#[test]
fn move_error_already_exists_is_conflict() {
    let io_err = std::io::Error::new(std::io::ErrorKind::AlreadyExists, "exists");
    let err = MoveError::from_io_error(io_err, Path::new("/test"));
    assert_eq!(err.user_descriptor().category(), ErrorCategory::Conflict);
}

#[test]
fn move_error_other_errors_are_unavailable() {
    let io_err = std::io::Error::other("other");
    let err = MoveError::from_io_error(io_err, Path::new("/test"));
    assert_eq!(err.user_descriptor().category(), ErrorCategory::Unavailable);
}

#[test]
fn move_error_invalid_target_category() {
    let err = MoveError::invalid_target("target directory is not a directory");
    assert_eq!(err.user_descriptor().category(), ErrorCategory::InvalidInput);
}

#[test]
fn move_error_collision_category() {
    let err = MoveError::collision();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::Conflict);
}

#[test]
fn move_error_cancelled_category() {
    let err = MoveError::cancelled();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::Cancelled);
}

#[test]
fn file_move_port_returns_new_paths() {
    let mover = FakeFileMove::new(Box::new(|files, target, _| {
        files
            .iter()
            .map(|path| {
                let name = path.file_name().expect("file name");
                (path.clone(), Ok(target.join(name)))
            })
            .collect()
    }));
    let cancelled = AtomicBool::new(false);
    let results = mover.move_files(
        &[PathBuf::from("/music/a.wav"), PathBuf::from("/music/b.wav")],
        Path::new("/library"),
        &cancelled,
    );
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].1.as_ref().unwrap(), &PathBuf::from("/library/a.wav"));
    assert_eq!(results[1].1.as_ref().unwrap(), &PathBuf::from("/library/b.wav"));
}

#[test]
fn file_move_port_propagates_error() {
    let mover = FakeFileMove::new(Box::new(|_, _, _| {
        vec![(PathBuf::from("/music/a.wav"), Err(MoveError::collision()))]
    }));
    let cancelled = AtomicBool::new(false);
    let results =
        mover.move_files(&[PathBuf::from("/music/a.wav")], Path::new("/library"), &cancelled);
    assert_eq!(
        results[0].1.as_ref().unwrap_err().user_descriptor().category(),
        ErrorCategory::Conflict
    );
}

#[test]
fn file_move_port_continues_after_partial_failure() {
    let mover = FakeFileMove::new(Box::new(|files, target, _| {
        files
            .iter()
            .enumerate()
            .map(|(index, path)| {
                if index == 0 {
                    let name = path.file_name().expect("file name");
                    (path.clone(), Ok(target.join(name)))
                } else {
                    (path.clone(), Err(MoveError::cancelled()))
                }
            })
            .collect()
    }));
    let cancelled = AtomicBool::new(false);
    let results = mover.move_files(
        &[PathBuf::from("/music/a.wav"), PathBuf::from("/music/b.wav")],
        Path::new("/library"),
        &cancelled,
    );
    assert!(results[0].1.is_ok());
    assert_eq!(
        results[1].1.as_ref().unwrap_err().user_descriptor().category(),
        ErrorCategory::Cancelled
    );
}

#[test]
fn file_move_port_receives_cancel_flag() {
    let seen = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let seen_for_closure = std::sync::Arc::clone(&seen);
    let mover = FakeFileMove::new(Box::new(move |_, _, cancelled| {
        seen_for_closure.fetch_add(cancelled.load(Ordering::Acquire) as usize, Ordering::SeqCst);
        vec![]
    }));
    let cancelled = AtomicBool::new(true);
    mover.move_files(&[PathBuf::from("/music/a.wav")], Path::new("/library"), &cancelled);
    assert_eq!(seen.load(Ordering::SeqCst), 1, "port must observe the cancel flag");
}
