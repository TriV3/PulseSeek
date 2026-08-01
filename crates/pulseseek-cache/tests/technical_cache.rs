use std::fs;
use std::path::PathBuf;

use pulseseek_cache::sqlite::{open_or_recover, Migration};
use pulseseek_cache::technical_cache::{CacheStatus, TechnicalCache, TechnicalCachePort};

fn cache_path(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("app-cache.sqlite")
}

fn start_cache(dir: &tempfile::TempDir) -> TechnicalCache {
    TechnicalCache::start(cache_path(dir)).expect("cache starts")
}

fn quarantine_files(dir: &tempfile::TempDir) -> Vec<PathBuf> {
    let mut names: Vec<PathBuf> = fs::read_dir(dir.path())
        .expect("read dir")
        .map(|entry| entry.expect("entry").path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.contains(".corrupt-")))
        .collect();
    names.sort();
    names
}

fn backup_files(dir: &tempfile::TempDir) -> Vec<PathBuf> {
    let mut names: Vec<PathBuf> = fs::read_dir(dir.path())
        .expect("read dir")
        .map(|entry| entry.expect("entry").path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.contains(".backup-")))
        .collect();
    names.sort();
    names
}

// ── Fresh migration ────────────────────────────────────────────────

#[test]
fn fresh_migration_creates_working_meta_store() {
    let dir = tempfile::tempdir().unwrap();
    let cache = start_cache(&dir);

    assert_eq!(
        cache.status(),
        CacheStatus::Healthy { schema_version: 2 },
        "fresh cache is healthy at schema 1"
    );

    cache.set_meta("theme", b"midnight".to_vec()).expect("set");
    assert_eq!(cache.get_meta("theme").expect("get"), Some(b"midnight".to_vec()));
    assert_eq!(cache.get_meta("missing").expect("get missing"), None);

    drop(cache);

    // Data survives a restart (repeat migration is a no-op).
    let reopened = start_cache(&dir);
    assert_eq!(reopened.get_meta("theme").expect("get after reopen"), Some(b"midnight".to_vec()));
}

#[test]
fn repeat_migration_is_noop_and_does_not_backup() {
    let dir = tempfile::tempdir().unwrap();
    let first = start_cache(&dir);
    assert_eq!(first.status(), CacheStatus::Healthy { schema_version: 2 });
    first.set_meta("k", b"v".to_vec()).expect("set");
    drop(first);

    let second = start_cache(&dir);
    assert_eq!(second.status(), CacheStatus::Healthy { schema_version: 2 });
    assert_eq!(second.get_meta("k").expect("get"), Some(b"v".to_vec()));
    drop(second);

    assert!(backup_files(&dir).is_empty(), "repeated migration must not create a backup");
}

// ── Migration rollback and backup (low-level runner) ───────────────

#[test]
fn failed_migration_rolls_back_and_keeps_previous_version() {
    let dir = tempfile::tempdir().unwrap();
    let path = cache_path(&dir);

    let v1 = Migration { version: 1, sql: "CREATE TABLE rollback_probe (id INTEGER PRIMARY KEY);" };
    let v2 = Migration { version: 2, sql: "THIS IS NOT VALID SQL" };

    open_or_recover(&path, &[v1]).expect("v1 applies");
    let err = open_or_recover(&path, &[v1, v2]).expect_err("v2 must fail");
    assert!(matches!(err, pulseseek_cache::sqlite::SqliteError::Migration { version: 2, .. }));

    let mut reopened = open_or_recover(&path, &[v1]).expect("reopen at v1");
    assert_eq!(reopened.schema_version(), 1);
    let exists: bool = reopened
        .connection()
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='rollback_probe')",
            [],
            |row| row.get(0),
        )
        .expect("query");
    assert!(exists, "v1 table remains after failed v2");
}

#[test]
fn migration_backs_up_existing_database() {
    let dir = tempfile::tempdir().unwrap();
    let path = cache_path(&dir);

    let v1 = Migration { version: 1, sql: "CREATE TABLE first (id INTEGER PRIMARY KEY);" };
    let v2 = Migration { version: 2, sql: "CREATE TABLE second (id INTEGER PRIMARY KEY);" };

    open_or_recover(&path, &[v1]).expect("v1 applies");
    assert!(backup_files(&dir).is_empty(), "fresh v1 must not back up");

    let upgraded = open_or_recover(&path, &[v1, v2]).expect("v2 applies");
    assert_eq!(upgraded.schema_version(), 2);

    let backups = backup_files(&dir);
    assert_eq!(backups.len(), 1, "v1 -> v2 must create one backup");
    assert!(backups[0]
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.contains("backup-1")));
}

// ── Corruption recovery ────────────────────────────────────────────

#[test]
fn corrupt_database_recovers_and_reports_degraded() {
    let dir = tempfile::tempdir().unwrap();
    let path = cache_path(&dir);
    fs::write(&path, b"this is definitely not a sqlite database").expect("write garbage");

    let cache = TechnicalCache::start(&path).expect("start recovers from corruption");
    assert_eq!(
        cache.status(),
        CacheStatus::Degraded {
            schema_version: 2,
            reason: "recovered from corrupt database".to_string()
        }
    );

    cache.set_meta("k", b"v".to_vec()).expect("set after recovery");
    assert_eq!(cache.get_meta("k").expect("get"), Some(b"v".to_vec()));

    assert_eq!(quarantine_files(&dir).len(), 1, "corrupt file is quarantined");
    drop(cache);

    // Subsequent opens are healthy.
    let reopened = start_cache(&dir);
    assert_eq!(reopened.status(), CacheStatus::Healthy { schema_version: 2 });
}

#[test]
fn corrupt_database_does_not_block_startup() {
    let dir = tempfile::tempdir().unwrap();
    let path = cache_path(&dir);
    fs::write(&path, b"garbage that is not a database").expect("write garbage");

    // Startup path must return a usable cache instead of failing hard.
    let cache = TechnicalCache::start(&path).expect("startup continues with cache");
    assert!(cache.get_meta("anything").is_ok());
}

#[test]
fn unwritable_path_fails_without_panicking() {
    let dir = tempfile::tempdir().unwrap();
    let locked = dir.path().join("locked");
    fs::create_dir_all(&locked).expect("create locked dir");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).expect("lock dir");
    }

    let result = TechnicalCache::start(locked.join("app-cache.sqlite"));
    assert!(result.is_err(), "unwritable path must fail cleanly");
}

// ── Worker meta semantics ──────────────────────────────────────────

#[test]
fn set_meta_upserts_and_remove_deletes() {
    let dir = tempfile::tempdir().unwrap();
    let cache = start_cache(&dir);

    cache.set_meta("key", b"one".to_vec()).expect("first set");
    cache.set_meta("key", b"two".to_vec()).expect("upsert");
    assert_eq!(cache.get_meta("key").expect("get"), Some(b"two".to_vec()));

    cache.remove_meta("key").expect("remove");
    assert_eq!(cache.get_meta("key").expect("get after remove"), None);
}

#[test]
fn blob_values_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let cache = start_cache(&dir);

    let payload: Vec<u8> = (0u8..=255).collect();
    cache.set_meta("blob", payload.clone()).expect("set blob");
    assert_eq!(cache.get_meta("blob").expect("get blob"), Some(payload));
}
