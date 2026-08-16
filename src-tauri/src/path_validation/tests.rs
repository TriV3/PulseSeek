use std::fs;

use super::*;
use tempfile::tempdir;

#[test]
fn valid_directory_returns_ok() {
    let dir = tempdir().expect("tempdir creation");
    assert!(validate_directory(dir.path().to_str().unwrap()).is_ok());
}

#[test]
fn nonexistent_path_fails() {
    let result = validate_directory("/tmp/pulseseek-nonexistent-xxxxxxxx");
    assert!(result.is_err());
}

#[test]
fn file_path_fails() {
    let dir = tempdir().expect("tempdir creation");
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "content").expect("write test file");
    let result = validate_directory(file_path.to_str().unwrap());
    assert!(result.is_err());
}

#[test]
#[cfg(unix)]
fn unreadable_directory_fails() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().expect("tempdir creation");
    let mut perms = fs::metadata(dir.path()).unwrap().permissions();
    perms.set_mode(0o000);
    fs::set_permissions(dir.path(), perms).expect("set permissions");

    if fs::read_dir(dir.path()).is_ok() {
        let mut perms = fs::metadata(dir.path()).unwrap().permissions();
        perms.set_mode(0o755);
        let _ = fs::set_permissions(dir.path(), perms);
        return;
    }

    let result = validate_directory(dir.path().to_str().unwrap());
    assert!(result.is_err());

    let mut perms = fs::metadata(dir.path()).unwrap().permissions();
    perms.set_mode(0o755);
    let _ = fs::set_permissions(dir.path(), perms);
}
