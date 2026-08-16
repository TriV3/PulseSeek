use std::path::PathBuf;

use pulseseek_cache::shortcut_mappings::{ShortcutMappingsCachePort, ShortcutMappingsError};
use pulseseek_cache::sqlite::{open_or_recover, Migration};
use pulseseek_cache::technical_cache::{TechnicalCache, CACHE_MIGRATIONS, CACHE_SCHEMA_VERSION};
use pulseseek_domain::shortcuts::{
    default_shortcut_mappings, Platform, ShortcutAction, ShortcutChord, ShortcutError,
    ShortcutMapping,
};

fn cache_path(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("app-cache.sqlite")
}

fn mapping(action: ShortcutAction, key: &str, primary: bool) -> ShortcutMapping {
    ShortcutMapping::new(action, ShortcutChord::new(key, primary, false, false))
}

#[test]
fn latest_schema_keeps_shortcut_defaults_available() {
    let dir = tempfile::tempdir().unwrap();
    let cache = TechnicalCache::start(cache_path(&dir)).expect("start cache");
    assert_eq!(CACHE_SCHEMA_VERSION, 5);

    cache.reset_shortcut_mappings(Platform::Linux).expect("reset defaults");
    assert_eq!(cache.load_shortcut_mappings().expect("load defaults"), default_shortcut_mappings());
}

#[test]
fn replace_is_full_set_normalized_and_persistent() {
    let dir = tempfile::tempdir().unwrap();
    let mut custom = default_shortcut_mappings();
    let open_folder = custom
        .iter_mut()
        .find(|mapping| mapping.action == ShortcutAction::OpenFolder)
        .expect("open folder default");
    open_folder.chord.key = " P ".to_string();
    {
        let cache = TechnicalCache::start(cache_path(&dir)).expect("start cache");
        cache.replace_shortcut_mappings(&custom, Platform::Linux).expect("replace mappings");
        assert_eq!(
            cache.load_shortcut_mappings().expect("load"),
            custom
                .iter()
                .map(|mapping| {
                    let mut mapping = mapping.clone();
                    mapping.chord.key = mapping.chord.key.trim().to_lowercase();
                    mapping
                })
                .collect::<Vec<_>>()
        );
    }

    let reopened = TechnicalCache::start(cache_path(&dir)).expect("reopen cache");
    assert_eq!(
        reopened.load_shortcut_mappings().expect("load persisted").len(),
        default_shortcut_mappings().len()
    );
}

#[test]
fn invalid_replace_keeps_previous_set() {
    let dir = tempfile::tempdir().unwrap();
    let cache = TechnicalCache::start(cache_path(&dir)).expect("start cache");
    let original = default_shortcut_mappings();
    cache.replace_shortcut_mappings(&original, Platform::Linux).expect("seed mappings");

    let conflicting = vec![
        mapping(ShortcutAction::Refresh, "x", true),
        mapping(ShortcutAction::FocusSearch, "X", true),
    ];
    assert!(matches!(
        cache.replace_shortcut_mappings(&conflicting, Platform::Linux),
        Err(ShortcutMappingsError::Validation(_))
    ));
    assert_eq!(cache.load_shortcut_mappings().expect("load original"), original);
}

#[test]
fn incomplete_replace_keeps_previous_complete_set() {
    let dir = tempfile::tempdir().unwrap();
    let cache = TechnicalCache::start(cache_path(&dir)).expect("start cache");
    let original = default_shortcut_mappings();
    cache.replace_shortcut_mappings(&original, Platform::Linux).expect("seed mappings");

    let incomplete = vec![mapping(ShortcutAction::Refresh, "f5", false)];
    assert!(matches!(
        cache.replace_shortcut_mappings(&incomplete, Platform::Linux),
        Err(ShortcutMappingsError::Validation(ShortcutError::IncompleteProfile))
    ));
    assert_eq!(cache.load_shortcut_mappings().expect("load original"), original);
}

#[test]
fn database_unique_chord_constraint_defends_direct_writes() {
    let dir = tempfile::tempdir().unwrap();
    let mut database = open_or_recover(&cache_path(&dir), &CACHE_MIGRATIONS).expect("open v4");
    database
        .connection()
        .execute(
            "INSERT INTO shortcut_mappings (action, key, primary_modifier, shift_modifier, alt_modifier) VALUES ('refresh', 'x', 1, 0, 0)",
            [],
        )
        .expect("first insert");
    let duplicate = database.connection().execute(
        "INSERT INTO shortcut_mappings (action, key, primary_modifier, shift_modifier, alt_modifier) VALUES ('focus_search', 'X', 1, 0, 0)",
        [],
    );
    assert!(duplicate.is_err());
}

#[test]
fn v3_upgrades_to_latest_without_losing_existing_data() {
    let dir = tempfile::tempdir().unwrap();
    let path = cache_path(&dir);
    open_or_recover(&path, &CACHE_MIGRATIONS[..3])
        .expect("create v3")
        .connection()
        .execute(
            "INSERT INTO recent_folders (path, name, last_opened_ms) VALUES ('/music', 'music', 1)",
            [],
        )
        .expect("seed v3 data");

    let mut upgraded = open_or_recover(&path, &CACHE_MIGRATIONS).expect("upgrade to v4");
    assert_eq!(upgraded.schema_version(), 5);
    let recent_count: i64 = upgraded
        .connection()
        .query_row("SELECT COUNT(*) FROM recent_folders", [], |row| row.get(0))
        .expect("count recent folders");
    assert_eq!(recent_count, 1);
}

#[test]
fn failed_v4_migration_rolls_back_table_and_version() {
    let dir = tempfile::tempdir().unwrap();
    let path = cache_path(&dir);
    let v3 = [
        Migration { version: 1, sql: "CREATE TABLE one (id INTEGER PRIMARY KEY);" },
        Migration { version: 2, sql: "CREATE TABLE two (id INTEGER PRIMARY KEY);" },
        Migration { version: 3, sql: "CREATE TABLE three (id INTEGER PRIMARY KEY);" },
    ];
    open_or_recover(&path, &v3).expect("create v3");
    let broken_v4 = Migration {
        version: 4,
        sql: "CREATE TABLE shortcut_mappings (action TEXT PRIMARY KEY); THIS IS INVALID SQL;",
    };
    assert!(open_or_recover(&path, &[v3[0], v3[1], v3[2], broken_v4]).is_err());

    let mut reopened = open_or_recover(&path, &v3).expect("reopen v3");
    assert_eq!(reopened.schema_version(), 3);
    let table_exists: bool = reopened
        .connection()
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='shortcut_mappings')",
            [],
            |row| row.get(0),
        )
        .expect("query table");
    assert!(!table_exists);
}
