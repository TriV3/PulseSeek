use std::path::{Path, PathBuf};

use super::*;

struct FakeFileTrash {
    f: Box<dyn Fn(&[PathBuf]) -> Vec<TrashResult> + Send>,
}

impl FakeFileTrash {
    fn new(f: Box<dyn Fn(&[PathBuf]) -> Vec<TrashResult> + Send>) -> Self {
        Self { f }
    }
}

impl FileTrash for FakeFileTrash {
    fn move_to_trash(&self, paths: &[PathBuf]) -> Vec<TrashResult> {
        (self.f)(paths)
    }
}

#[test]
fn trash_error_implements_error_contract() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "test");
    let err = TrashError::from_io_error(io_err, Path::new("/test"));
    let desc = err.user_descriptor();
    assert!(!desc.message().is_empty(), "error should have a safe message");
    assert_eq!(err.diagnostic_context().code(), "file.operation");
}

#[test]
fn trash_error_permission_denied_category() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
    let err = TrashError::from_io_error(io_err, Path::new("/test"));
    assert_eq!(err.user_descriptor().category(), ErrorCategory::PermissionDenied);
}

#[test]
fn trash_error_not_found_category() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
    let err = TrashError::from_io_error(io_err, Path::new("/test"));
    assert_eq!(err.user_descriptor().category(), ErrorCategory::NotFound);
}

#[test]
fn trash_error_other_errors_are_unavailable() {
    let io_err = std::io::Error::other("other");
    let err = TrashError::from_io_error(io_err, Path::new("/test"));
    assert_eq!(err.user_descriptor().category(), ErrorCategory::Unavailable);
}

#[test]
fn trash_error_cancelled_has_cancelled_category() {
    let err = TrashError::cancelled();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::Cancelled);
}

#[test]
fn file_trash_returns_one_result_per_input_path() {
    let paths = vec![PathBuf::from("/music/a.wav"), PathBuf::from("/music/b.wav")];
    let paths_copy = paths.clone();
    let trash = FakeFileTrash::new(Box::new(move |_| {
        paths_copy.iter().map(|p| (p.clone(), Ok(()))).collect()
    }));
    let results = trash.move_to_trash(&paths);
    assert_eq!(results.len(), 2, "should return one result per input path");
    assert!(results.iter().all(|(_, r)| r.is_ok()), "all should succeed");
}

#[test]
fn file_trash_reports_partial_batch_failure() {
    let paths = vec![
        PathBuf::from("/music/good.wav"),
        PathBuf::from("/music/bad.wav"),
        PathBuf::from("/music/also_good.wav"),
    ];
    let trash = FakeFileTrash::new(Box::new(|_| {
        vec![
            (PathBuf::from("/music/good.wav"), Ok(())),
            (
                PathBuf::from("/music/bad.wav"),
                Err(TrashError::from_io_error(
                    std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
                    Path::new("/music/bad.wav"),
                )),
            ),
            (PathBuf::from("/music/also_good.wav"), Ok(())),
        ]
    }));
    let results = trash.move_to_trash(&paths);
    assert_eq!(results.len(), 3);
    assert!(results[0].1.is_ok());
    assert!(results[1].1.is_err());
    assert_eq!(
        results[1].1.as_ref().unwrap_err().user_descriptor().category(),
        ErrorCategory::NotFound
    );
    assert!(results[2].1.is_ok());
}

#[test]
fn file_trash_accepts_empty_list() {
    let trash = FakeFileTrash::new(Box::new(|_| vec![]));
    let results = trash.move_to_trash(&[]);
    assert!(results.is_empty(), "empty input yields empty results");
}
