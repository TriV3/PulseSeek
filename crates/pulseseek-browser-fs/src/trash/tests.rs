use std::path::PathBuf;

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
    assert!(results[0].1.is_ok(), "good file should be trashed");
    assert!(results[1].1.is_err(), "missing file should error");
}

#[test]
fn native_trash_empty_list_returns_empty() {
    let results = NativeFileTrash.move_to_trash(&[]);
    assert!(results.is_empty());
}
