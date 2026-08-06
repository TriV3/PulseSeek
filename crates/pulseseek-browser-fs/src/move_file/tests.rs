use std::sync::atomic::AtomicBool;

use pulseseek_domain::error::{ErrorCategory, ErrorContract};
use tempfile::tempdir;

use super::*;

#[test]
fn native_mover_moves_file_into_target_directory() {
    let dir = tempdir().expect("tempdir");
    let target_dir = dir.path().join("library");
    std::fs::create_dir(&target_dir).expect("create target dir");
    let source = dir.path().join("track.wav");
    std::fs::write(&source, "content").expect("write test file");
    let cancelled = AtomicBool::new(false);

    let results =
        NativeFileMover::new().move_files(std::slice::from_ref(&source), &target_dir, &cancelled);

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1.as_ref().unwrap(), &target_dir.join("track.wav"));
    assert!(!source.exists(), "old path should be gone after move");
    assert_eq!(
        std::fs::read_to_string(target_dir.join("track.wav")).expect("read moved file"),
        "content"
    );
}

#[test]
fn native_mover_same_directory_move_is_noop_success() {
    let dir = tempdir().expect("tempdir");
    let source = dir.path().join("track.wav");
    std::fs::write(&source, "content").expect("write test file");
    let cancelled = AtomicBool::new(false);

    let results =
        NativeFileMover::new().move_files(std::slice::from_ref(&source), dir.path(), &cancelled);

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1.as_ref().unwrap(), &source);
    assert!(source.exists(), "file should remain untouched");
}

#[test]
fn native_mover_rejects_collision_without_overwriting() {
    let dir = tempdir().expect("tempdir");
    let target_dir = dir.path().join("library");
    std::fs::create_dir(&target_dir).expect("create target dir");
    let source = dir.path().join("track.wav");
    std::fs::write(&source, "content").expect("write test file");
    std::fs::write(target_dir.join("track.wav"), "other").expect("write existing file");
    let cancelled = AtomicBool::new(false);

    let results =
        NativeFileMover::new().move_files(std::slice::from_ref(&source), &target_dir, &cancelled);

    assert_eq!(
        results[0].1.as_ref().unwrap_err().user_descriptor().category(),
        ErrorCategory::Conflict
    );
    assert!(source.exists(), "source must not be removed on collision");
    assert_eq!(
        std::fs::read_to_string(target_dir.join("track.wav")).expect("read existing"),
        "other",
        "existing file must not be overwritten"
    );
}

#[test]
fn native_mover_missing_source_returns_not_found() {
    let dir = tempdir().expect("tempdir");
    let target_dir = dir.path().join("library");
    std::fs::create_dir(&target_dir).expect("create target dir");
    let missing = dir.path().join("missing.wav");
    let cancelled = AtomicBool::new(false);

    let results = NativeFileMover::new().move_files(&[missing], &target_dir, &cancelled);

    assert_eq!(
        results[0].1.as_ref().unwrap_err().user_descriptor().category(),
        ErrorCategory::NotFound
    );
}

#[test]
fn native_mover_rejects_directory_source() {
    let dir = tempdir().expect("tempdir");
    let target_dir = dir.path().join("library");
    std::fs::create_dir(&target_dir).expect("create target dir");
    let source_dir = dir.path().join("songs");
    std::fs::create_dir(&source_dir).expect("create source dir");
    let cancelled = AtomicBool::new(false);

    let results = NativeFileMover::new().move_files(&[source_dir], &target_dir, &cancelled);

    assert_eq!(
        results[0].1.as_ref().unwrap_err().user_descriptor().category(),
        ErrorCategory::InvalidInput
    );
}

#[test]
fn native_mover_missing_target_directory_rejected() {
    let dir = tempdir().expect("tempdir");
    let source = dir.path().join("track.wav");
    std::fs::write(&source, "content").expect("write test file");
    let missing_target = dir.path().join("missing");
    let cancelled = AtomicBool::new(false);

    let results = NativeFileMover::new().move_files(
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
fn native_mover_cancelled_before_start_reports_cancelled() {
    let dir = tempdir().expect("tempdir");
    let target_dir = dir.path().join("library");
    std::fs::create_dir(&target_dir).expect("create target dir");
    let source = dir.path().join("track.wav");
    std::fs::write(&source, "content").expect("write test file");
    let cancelled = AtomicBool::new(true);

    let results =
        NativeFileMover::new().move_files(std::slice::from_ref(&source), &target_dir, &cancelled);

    assert_eq!(
        results[0].1.as_ref().unwrap_err().user_descriptor().category(),
        ErrorCategory::Cancelled
    );
    assert!(source.exists(), "cancelled move must not touch the source");
}

#[test]
fn native_mover_continues_after_partial_failure() {
    let dir = tempdir().expect("tempdir");
    let target_dir = dir.path().join("library");
    std::fs::create_dir(&target_dir).expect("create target dir");
    let source_ok = dir.path().join("ok.wav");
    let source_blocked = dir.path().join("blocked.wav");
    std::fs::write(&source_ok, "one").expect("write first file");
    std::fs::write(&source_blocked, "two").expect("write second file");
    std::fs::write(target_dir.join("blocked.wav"), "existing").expect("write collision target");
    let cancelled = AtomicBool::new(false);

    let results = NativeFileMover::new().move_files(
        &[source_ok.clone(), source_blocked.clone()],
        &target_dir,
        &cancelled,
    );

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].1.as_ref().unwrap(), &target_dir.join("ok.wav"));
    assert_eq!(
        results[1].1.as_ref().unwrap_err().user_descriptor().category(),
        ErrorCategory::Conflict
    );
    assert!(!source_ok.exists(), "first file should have moved");
    assert!(source_blocked.exists(), "blocked source must remain");
}

#[test]
fn native_mover_cross_device_uses_copy_fallback() {
    let dir = tempdir().expect("tempdir");
    let target_dir = dir.path().join("library");
    std::fs::create_dir(&target_dir).expect("create target dir");
    let source = dir.path().join("track.wav");
    std::fs::write(&source, "content").expect("write test file");
    let cancelled = AtomicBool::new(false);
    let mover = NativeFileMover::with_rename(Box::new(|_, _| {
        Err(std::io::Error::new(
            std::io::ErrorKind::CrossesDevices,
            "simulated cross-device rename",
        ))
    }));

    let results = mover.move_files(std::slice::from_ref(&source), &target_dir, &cancelled);

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1.as_ref().unwrap(), &target_dir.join("track.wav"));
    assert!(!source.exists(), "source should be removed after the copy");
    assert_eq!(
        std::fs::read_to_string(target_dir.join("track.wav")).expect("read moved file"),
        "content"
    );
}

#[test]
fn native_mover_cross_device_copy_failure_keeps_source() {
    let dir = tempdir().expect("tempdir");
    let target_dir = dir.path().join("library");
    std::fs::create_dir(&target_dir).expect("create target dir");
    let source = dir.path().join("track.wav");
    std::fs::write(&source, "content").expect("write test file");
    let cancelled = AtomicBool::new(false);
    // Force the fallback copy to fail by making the target directory
    // read-only, then restore permissions so tempdir cleanup succeeds.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&target_dir, std::fs::Permissions::from_mode(0o500))
            .expect("set read-only target dir");
    }
    let mover = NativeFileMover::with_rename(Box::new(|_, _| {
        Err(std::io::Error::new(
            std::io::ErrorKind::CrossesDevices,
            "simulated cross-device rename",
        ))
    }));

    let results = mover.move_files(std::slice::from_ref(&source), &target_dir, &cancelled);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&target_dir, std::fs::Permissions::from_mode(0o700))
            .expect("restore target dir permissions");
    }
    assert_eq!(
        results[0].1.as_ref().unwrap_err().user_descriptor().category(),
        ErrorCategory::PermissionDenied
    );
    assert!(source.exists(), "source must remain when the copy fails");
    assert!(
        !target_dir.join("track.wav").exists(),
        "no partial target may remain after a failed copy"
    );
}

#[test]
fn copy_then_remove_moves_content_across_directories() {
    let dir = tempdir().expect("tempdir");
    let target_dir = dir.path().join("library");
    std::fs::create_dir(&target_dir).expect("create target dir");
    let source = dir.path().join("track.wav");
    std::fs::write(&source, "content").expect("write test file");
    let target = target_dir.join("track.wav");

    let result = copy_then_remove(&source, &target).expect("fallback succeeds");

    assert_eq!(result, target);
    assert!(!source.exists(), "source removed after successful copy");
    assert_eq!(std::fs::read_to_string(&target).expect("read moved file"), "content");
}

#[test]
#[cfg(unix)]
fn native_mover_permission_denied_maps_to_permission_error() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().expect("tempdir");
    let target_dir = dir.path().join("library");
    std::fs::create_dir(&target_dir).expect("create target dir");
    let source = dir.path().join("track.wav");
    std::fs::write(&source, "content").expect("write test file");

    // Remove write permission from the parent directory so the rename fails.
    let read_only = std::fs::Permissions::from_mode(0o500);
    std::fs::set_permissions(dir.path(), read_only).expect("set read-only parent");
    let cancelled = AtomicBool::new(false);
    let results =
        NativeFileMover::new().move_files(std::slice::from_ref(&source), &target_dir, &cancelled);

    // Restore permissions before asserting so tempdir cleanup can proceed.
    let writable = std::fs::Permissions::from_mode(0o700);
    std::fs::set_permissions(dir.path(), writable).expect("restore parent permissions");

    assert_eq!(
        results[0].1.as_ref().unwrap_err().user_descriptor().category(),
        ErrorCategory::PermissionDenied
    );
    assert!(source.exists(), "source should remain when rename fails");
}
