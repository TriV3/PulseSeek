use std::fs;
use std::path::PathBuf;

use pulseseek_cache::recent_folders::{RecentFolder, RecentFoldersCachePort, RECENT_FOLDERS_LIMIT};
use pulseseek_cache::sqlite::{open_or_recover, OpenedDatabase};
use pulseseek_cache::technical_cache::{TechnicalCache, CACHE_MIGRATIONS};

fn cache_path(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("app-cache.sqlite")
}

fn start_cache(dir: &tempfile::TempDir) -> TechnicalCache {
    TechnicalCache::start(cache_path(dir)).expect("cache starts")
}

fn open_raw(dir: &tempfile::TempDir) -> OpenedDatabase {
    open_or_recover(&cache_path(dir), &CACHE_MIGRATIONS).expect("open cache database")
}

fn recent_row_count(dir: &tempfile::TempDir) -> usize {
    let mut database = open_raw(dir);
    let count: i64 = database
        .connection()
        .query_row("SELECT COUNT(*) FROM recent_folders", [], |row| row.get(0))
        .expect("count recent folder rows");
    count as usize
}

fn paths(cache: &TechnicalCache) -> Vec<String> {
    cache
        .list_recent_folders()
        .expect("list recent folders")
        .into_iter()
        .map(|folder| folder.path)
        .collect()
}

// ── Add ──────────────────────────────────────────────────────────────

#[test]
fn record_adds_folder_with_derived_name() {
    let dir = tempfile::tempdir().unwrap();
    let cache = start_cache(&dir);

    cache.record_recent_folder("/music/project").expect("record");

    let folders = cache.list_recent_folders().expect("list");
    assert_eq!(folders.len(), 1);
    assert_eq!(
        folders[0],
        RecentFolder {
            path: "/music/project".to_string(),
            name: "project".to_string(),
            last_opened_ms: folders[0].last_opened_ms,
        }
    );
    assert!(folders[0].last_opened_ms > 0, "records an opened timestamp");
}

// ── Reorder ──────────────────────────────────────────────────────────

#[test]
fn re_recording_moves_folder_to_front() {
    let dir = tempfile::tempdir().unwrap();
    let cache = start_cache(&dir);

    cache.record_recent_folder("/music/one").expect("record one");
    cache.record_recent_folder("/music/two").expect("record two");
    assert_eq!(paths(&cache), ["/music/two", "/music/one"]);

    cache.record_recent_folder("/music/one").expect("re-record one");
    let folders = cache.list_recent_folders().expect("list");
    assert_eq!(paths(&cache), ["/music/one", "/music/two"]);
    assert!(
        folders[0].last_opened_ms >= folders[1].last_opened_ms,
        "re-opened folder has a fresh opened timestamp"
    );
}

// ── Limit ────────────────────────────────────────────────────────────

#[test]
fn history_is_bounded_to_recent_folders_limit() {
    let dir = tempfile::tempdir().unwrap();
    let cache = start_cache(&dir);

    for index in 0..(RECENT_FOLDERS_LIMIT + 5) {
        cache.record_recent_folder(&format!("/music/folder-{index:02}")).expect("record");
    }

    let folders = cache.list_recent_folders().expect("list");
    assert_eq!(folders.len(), RECENT_FOLDERS_LIMIT, "history stays bounded");
    assert_eq!(
        folders.first().map(|folder| folder.path.as_str()),
        Some("/music/folder-14"),
        "newest folder is first"
    );
    assert!(
        !folders.iter().any(|folder| folder.path == "/music/folder-04"),
        "oldest entry is evicted"
    );
    assert_eq!(recent_row_count(&dir), RECENT_FOLDERS_LIMIT, "rows are trimmed");
}

// ── Missing folder ───────────────────────────────────────────────────

#[test]
fn store_never_touches_disk_and_survives_deleted_folder() {
    let dir = tempfile::tempdir().unwrap();
    let cache = start_cache(&dir);

    // The store records a path without requiring it to exist: it is technical
    // cache data, not a filesystem probe.
    cache.record_recent_folder("/music/vanished").expect("record missing path");

    // A folder that existed when recorded and is later deleted stays in the
    // history so the UI can attempt a graceful reopen and report the failure.
    let existing = dir.path().join("project");
    fs::create_dir(&existing).expect("create project");
    cache.record_recent_folder(existing.to_str().expect("utf8 path")).expect("record existing");
    fs::remove_dir(&existing).expect("delete project");

    assert_eq!(
        paths(&cache),
        [existing.to_string_lossy().to_string(), "/music/vanished".to_string()],
        "history keeps both entries"
    );
}

// ── Clear history ────────────────────────────────────────────────────

#[test]
fn clear_removes_every_record() {
    let dir = tempfile::tempdir().unwrap();
    let cache = start_cache(&dir);

    cache.record_recent_folder("/music/one").expect("record one");
    cache.record_recent_folder("/music/two").expect("record two");
    cache.clear_recent_folders().expect("clear");

    assert!(paths(&cache).is_empty(), "history is empty after clear");
    assert_eq!(recent_row_count(&dir), 0, "rows are deleted");
}

#[test]
fn history_survives_cache_restart() {
    let dir = tempfile::tempdir().unwrap();
    {
        let cache = start_cache(&dir);
        cache.record_recent_folder("/music/persist").expect("record");
    }

    let reopened = start_cache(&dir);
    assert_eq!(paths(&reopened), ["/music/persist"]);
}
