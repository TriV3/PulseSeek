use super::*;
use pulseseek_domain::error::{ErrorCategory, ErrorContract};
use tempfile::tempdir;

#[test]
fn native_rename_moves_file_within_directory() {
    let dir = tempdir().expect("tempdir");
    let source = dir.path().join("track.wav");
    std::fs::write(&source, "content").expect("write test file");

    let target = NativeFileRename.rename(&source, "renamed.wav").expect("rename succeeds");

    assert_eq!(target, dir.path().join("renamed.wav"));
    assert!(!source.exists(), "old path should be gone after rename");
    assert!(target.exists(), "new path should exist after rename");
}

#[test]
fn native_rename_same_name_is_a_noop_success() {
    let dir = tempdir().expect("tempdir");
    let source = dir.path().join("track.wav");
    std::fs::write(&source, "content").expect("write test file");

    let target = NativeFileRename.rename(&source, "track.wav").expect("same-name rename succeeds");

    assert_eq!(target, source);
    assert!(source.exists(), "file should remain untouched");
}

#[test]
fn native_rename_rejects_collision() {
    let dir = tempdir().expect("tempdir");
    let source = dir.path().join("track.wav");
    let existing = dir.path().join("existing.wav");
    std::fs::write(&source, "content").expect("write test file");
    std::fs::write(&existing, "other").expect("write existing file");

    let error =
        NativeFileRename.rename(&source, "existing.wav").expect_err("collision should fail");
    assert_eq!(error.user_descriptor().category(), ErrorCategory::Conflict);
    assert!(source.exists(), "source must not be removed on collision");
    assert_eq!(
        std::fs::read_to_string(&existing).expect("read existing"),
        "other",
        "existing file must not be overwritten"
    );
}

#[test]
fn native_rename_rejects_invalid_names() {
    let dir = tempdir().expect("tempdir");
    let source = dir.path().join("track.wav");
    std::fs::write(&source, "content").expect("write test file");

    for name in ["", "sub/name.wav", ".", ".."] {
        let error = NativeFileRename.rename(&source, name).expect_err("invalid name should fail");
        assert_eq!(error.user_descriptor().category(), ErrorCategory::InvalidInput);
        assert!(source.exists(), "source must remain for invalid name {name:?}");
    }
}

#[test]
fn native_rename_missing_source_returns_not_found() {
    let dir = tempdir().expect("tempdir");
    let missing = dir.path().join("missing.wav");

    let error =
        NativeFileRename.rename(&missing, "renamed.wav").expect_err("missing source should fail");
    assert_eq!(error.user_descriptor().category(), ErrorCategory::NotFound);
}

#[test]
fn native_rename_source_removed_externally_returns_not_found() {
    // External race: the file disappears between validation and the rename.
    let dir = tempdir().expect("tempdir");
    let source = dir.path().join("track.wav");
    std::fs::write(&source, "content").expect("write test file");
    std::fs::remove_file(&source).expect("simulate external removal");

    let error = NativeFileRename
        .rename(&source, "renamed.wav")
        .expect_err("externally removed source should fail");
    assert_eq!(error.user_descriptor().category(), ErrorCategory::NotFound);
}

#[test]
#[cfg(unix)]
fn native_rename_permission_denied_maps_to_permission_error() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().expect("tempdir");
    let source = dir.path().join("track.wav");
    std::fs::write(&source, "content").expect("write test file");

    // Remove write permission from the parent directory so the rename fails.
    let read_only = std::fs::Permissions::from_mode(0o500);
    std::fs::set_permissions(dir.path(), read_only).expect("set read-only parent");
    let result = NativeFileRename.rename(&source, "renamed.wav");

    // Restore permissions before asserting so tempdir cleanup can proceed.
    let writable = std::fs::Permissions::from_mode(0o700);
    std::fs::set_permissions(dir.path(), writable).expect("restore parent permissions");

    let error = result.expect_err("read-only parent should fail");
    assert_eq!(error.user_descriptor().category(), ErrorCategory::PermissionDenied);
    assert!(source.exists(), "source should remain when rename fails");
}
