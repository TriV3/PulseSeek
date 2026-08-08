use super::*;
use tempfile::tempdir;

#[test]
fn trash_batch_moves_file_through_backend() {
    let dir = tempdir().expect("tempdir");
    let trash_dir = tempdir().expect("test trash dir");
    let file_path = dir.path().join("to_trash.txt");
    std::fs::write(&file_path, "content").expect("write test file");
    assert!(file_path.exists(), "file should exist before trash");

    let results = move_each_to_trash(std::slice::from_ref(&file_path), |path| {
        move_to_test_trash(path, trash_dir.path())
    });
    assert_eq!(results.len(), 1);
    assert!(results[0].1.is_ok(), "file should be trashed successfully: {:?}", results[0].1);
    assert!(!file_path.exists(), "file should be removed after trash");
    assert!(trash_dir.path().join("to_trash.txt").exists(), "test backend retains the file");
}

#[test]
fn native_trash_nonexistent_file_returns_error() {
    let dir = tempdir().expect("tempdir");
    let missing = dir.path().join("missing.wav");
    let results = NativeFileTrash.move_to_trash(&[missing]);
    assert_eq!(results.len(), 1);
    assert!(results[0].1.is_err(), "trashing nonexistent file should produce an error");
}

#[test]
fn trash_batch_reports_partial_backend_failure() {
    let dir = tempdir().expect("tempdir");
    let trash_dir = tempdir().expect("test trash dir");
    let good = dir.path().join("good.wav");
    let missing = dir.path().join("missing.wav");
    std::fs::write(&good, "content").expect("write test file");

    let results = move_each_to_trash(&[good.clone(), missing], |path| {
        move_to_test_trash(path, trash_dir.path())
    });
    assert_eq!(results.len(), 2);
    assert!(results[0].1.is_ok(), "good file should be trashed");
    assert!(results[1].1.is_err(), "missing file should error");
    assert!(trash_dir.path().join("good.wav").exists(), "successful item is retained");
}

#[test]
fn native_trash_empty_list_returns_empty() {
    let results = move_each_to_trash(&[], |_| unreachable!("empty batch must not call backend"));
    assert!(results.is_empty());
}

fn move_to_test_trash(
    path: &std::path::Path,
    trash_dir: &std::path::Path,
) -> Result<(), TrashError> {
    let file_name = path.file_name().ok_or_else(|| {
        TrashError::from_io_error(std::io::Error::other("test path has no file name"), path)
    })?;
    std::fs::rename(path, trash_dir.join(file_name))
        .map_err(|error| TrashError::from_io_error(error, path))
}
