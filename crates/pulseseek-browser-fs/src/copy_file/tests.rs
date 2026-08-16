use std::sync::atomic::AtomicBool;

use pulseseek_domain::error::{ErrorCategory, ErrorContract};
use tempfile::tempdir;

use super::*;

#[test]
fn native_copier_copies_file_into_target_directory() {
    let dir = tempdir().expect("tempdir");
    let target_dir = dir.path().join("library");
    std::fs::create_dir(&target_dir).expect("create target dir");
    let source = dir.path().join("track.wav");
    std::fs::write(&source, "content").expect("write test file");
    let cancelled = AtomicBool::new(false);

    let results =
        NativeFileCopier::new().copy_files(std::slice::from_ref(&source), &target_dir, &cancelled);

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1.as_ref().unwrap(), &target_dir.join("track.wav"));
    assert!(source.exists(), "original must remain after a copy");
    assert_eq!(
        std::fs::read_to_string(target_dir.join("track.wav")).expect("read copied file"),
        "content"
    );
}

#[test]
fn native_copier_rejects_collision_without_overwriting() {
    let dir = tempdir().expect("tempdir");
    let target_dir = dir.path().join("library");
    std::fs::create_dir(&target_dir).expect("create target dir");
    let source = dir.path().join("track.wav");
    std::fs::write(&source, "content").expect("write test file");
    std::fs::write(target_dir.join("track.wav"), "other").expect("write existing file");
    let cancelled = AtomicBool::new(false);

    let results =
        NativeFileCopier::new().copy_files(std::slice::from_ref(&source), &target_dir, &cancelled);

    assert_eq!(
        results[0].1.as_ref().unwrap_err().user_descriptor().category(),
        ErrorCategory::Conflict
    );
    assert!(source.exists(), "source must remain on collision");
    assert_eq!(
        std::fs::read_to_string(target_dir.join("track.wav")).expect("read existing"),
        "other",
        "existing target must never be overwritten"
    );
}

#[test]
fn native_copier_reports_missing_source() {
    let dir = tempdir().expect("tempdir");
    let target_dir = dir.path().join("library");
    std::fs::create_dir(&target_dir).expect("create target dir");
    let missing = dir.path().join("missing.wav");
    let cancelled = AtomicBool::new(false);

    let results = NativeFileCopier::new().copy_files(&[missing], &target_dir, &cancelled);

    assert_eq!(
        results[0].1.as_ref().unwrap_err().user_descriptor().category(),
        ErrorCategory::NotFound
    );
}

#[test]
fn native_copier_rejects_directory_source() {
    let dir = tempdir().expect("tempdir");
    let target_dir = dir.path().join("library");
    std::fs::create_dir(&target_dir).expect("create target dir");
    let source_dir = dir.path().join("songs");
    std::fs::create_dir(&source_dir).expect("create source dir");
    let cancelled = AtomicBool::new(false);

    let results = NativeFileCopier::new().copy_files(&[source_dir], &target_dir, &cancelled);

    assert_eq!(
        results[0].1.as_ref().unwrap_err().user_descriptor().category(),
        ErrorCategory::InvalidInput
    );
}

#[test]
fn native_copier_rejects_missing_target_directory() {
    let dir = tempdir().expect("tempdir");
    let source = dir.path().join("track.wav");
    std::fs::write(&source, "content").expect("write test file");
    let missing_target = dir.path().join("missing");
    let cancelled = AtomicBool::new(false);

    let results = NativeFileCopier::new().copy_files(
        std::slice::from_ref(&source),
        &missing_target,
        &cancelled,
    );

    assert_eq!(
        results[0].1.as_ref().unwrap_err().user_descriptor().category(),
        ErrorCategory::InvalidInput
    );
    assert!(source.exists(), "source must remain when target is invalid");
}

#[test]
fn native_copier_rejects_copy_into_own_directory() {
    let dir = tempdir().expect("tempdir");
    let source = dir.path().join("track.wav");
    std::fs::write(&source, "content").expect("write test file");
    let cancelled = AtomicBool::new(false);

    let results =
        NativeFileCopier::new().copy_files(std::slice::from_ref(&source), dir.path(), &cancelled);

    assert_eq!(
        results[0].1.as_ref().unwrap_err().user_descriptor().category(),
        ErrorCategory::Conflict
    );
    assert_eq!(
        std::fs::read_to_string(&source).expect("read source"),
        "content",
        "source must remain untouched when target equals source"
    );
}

#[test]
fn native_copier_cancelled_before_start_reports_cancelled() {
    let dir = tempdir().expect("tempdir");
    let target_dir = dir.path().join("library");
    std::fs::create_dir(&target_dir).expect("create target dir");
    let source = dir.path().join("track.wav");
    std::fs::write(&source, "content").expect("write test file");
    let cancelled = AtomicBool::new(true);

    let results =
        NativeFileCopier::new().copy_files(std::slice::from_ref(&source), &target_dir, &cancelled);

    assert_eq!(
        results[0].1.as_ref().unwrap_err().user_descriptor().category(),
        ErrorCategory::Cancelled
    );
    assert!(source.exists(), "cancelled copy must not touch the source");
}

#[test]
fn native_copier_continues_after_partial_failure() {
    let dir = tempdir().expect("tempdir");
    let target_dir = dir.path().join("library");
    std::fs::create_dir(&target_dir).expect("create target dir");
    let source_ok = dir.path().join("ok.wav");
    let source_blocked = dir.path().join("blocked.wav");
    std::fs::write(&source_ok, "one").expect("write first file");
    std::fs::write(&source_blocked, "two").expect("write second file");
    std::fs::write(target_dir.join("blocked.wav"), "existing").expect("write collision target");
    let cancelled = AtomicBool::new(false);

    let results = NativeFileCopier::new().copy_files(
        &[source_ok.clone(), source_blocked.clone()],
        &target_dir,
        &cancelled,
    );

    assert_eq!(results.len(), 2);
    assert!(results[0].1.is_ok(), "first file should copy");
    assert_eq!(
        results[1].1.as_ref().unwrap_err().user_descriptor().category(),
        ErrorCategory::Conflict
    );
    assert!(source_ok.exists(), "copied source must remain");
    assert!(source_blocked.exists(), "blocked source must remain");
}

#[cfg(unix)]
#[test]
fn native_copier_permission_denied_maps_to_permission_error() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().expect("tempdir");
    let target_dir = dir.path().join("library");
    std::fs::create_dir(&target_dir).expect("create target dir");
    let source = dir.path().join("track.wav");
    std::fs::write(&source, "content").expect("write test file");

    // Remove write permission from the target directory so the copy fails.
    let read_only = std::fs::Permissions::from_mode(0o500);
    std::fs::set_permissions(&target_dir, read_only).expect("set read-only target dir");
    let cancelled = AtomicBool::new(false);
    let results =
        NativeFileCopier::new().copy_files(std::slice::from_ref(&source), &target_dir, &cancelled);

    // Restore permissions before asserting so tempdir cleanup can proceed.
    let writable = std::fs::Permissions::from_mode(0o700);
    std::fs::set_permissions(&target_dir, writable).expect("restore target permissions");

    assert_eq!(
        results[0].1.as_ref().unwrap_err().user_descriptor().category(),
        ErrorCategory::PermissionDenied
    );
    assert!(source.exists(), "source should remain when copy fails");
    assert!(
        !target_dir.join("track.wav").exists(),
        "no partial target may remain after a failed copy"
    );
}
