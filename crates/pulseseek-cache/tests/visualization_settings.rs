use std::path::PathBuf;

use pulseseek_cache::sqlite::open_or_recover;
use pulseseek_cache::technical_cache::{TechnicalCache, CACHE_MIGRATIONS, CACHE_SCHEMA_VERSION};
use pulseseek_cache::visualization_settings::VisualizationSettingsCachePort;
use pulseseek_domain::visualization::{
    VisualizationMode, VisualizationQuality, VisualizationSettings,
};

fn cache_path(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("app-cache.sqlite")
}

#[test]
fn fresh_schema_is_v5_and_missing_settings_use_no_record() {
    let dir = tempfile::tempdir().unwrap();
    let cache = TechnicalCache::start(cache_path(&dir)).expect("start cache");

    assert_eq!(CACHE_SCHEMA_VERSION, 5);
    assert_eq!(cache.load_visualization_settings().unwrap(), None);
}

#[test]
fn visualization_settings_round_trip_across_restart() {
    let dir = tempfile::tempdir().unwrap();
    let settings =
        VisualizationSettings::new(false, VisualizationMode::Musical, VisualizationQuality::High);
    {
        let cache = TechnicalCache::start(cache_path(&dir)).expect("start cache");
        cache.save_visualization_settings(settings).expect("save settings");
        assert_eq!(cache.load_visualization_settings().unwrap(), Some(settings));
    }

    let reopened = TechnicalCache::start(cache_path(&dir)).expect("reopen cache");
    assert_eq!(reopened.load_visualization_settings().unwrap(), Some(settings));
}

#[test]
fn v4_upgrades_without_losing_existing_cache_data() {
    let dir = tempfile::tempdir().unwrap();
    let path = cache_path(&dir);
    open_or_recover(&path, &CACHE_MIGRATIONS[..4])
        .expect("create v4")
        .connection()
        .execute(
            "INSERT INTO cache_meta (key, value, updated_at_ms) VALUES ('theme', X'01', 1)",
            [],
        )
        .expect("seed v4 data");

    let mut upgraded = open_or_recover(&path, &CACHE_MIGRATIONS).expect("upgrade to v5");
    assert_eq!(upgraded.schema_version(), 5);
    let count: i64 = upgraded
        .connection()
        .query_row("SELECT COUNT(*) FROM cache_meta", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn database_constraints_reject_unknown_modes_and_quality() {
    let dir = tempfile::tempdir().unwrap();
    let mut database = open_or_recover(&cache_path(&dir), &CACHE_MIGRATIONS).unwrap();
    let invalid = database.connection().execute(
        "INSERT INTO visualization_settings (singleton, enabled, mode, quality) \
         VALUES (1, 1, 'plugin', 'extreme')",
        [],
    );
    assert!(invalid.is_err());
}
