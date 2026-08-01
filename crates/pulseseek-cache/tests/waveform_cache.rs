use std::fs;
use std::path::{Path, PathBuf};

use pulseseek_cache::sqlite::{open_or_recover, OpenedDatabase};
use pulseseek_cache::technical_cache::{
    CacheStatus, TechnicalCache, TechnicalCachePort, CACHE_MIGRATIONS,
};
use pulseseek_cache::waveform_cache::{
    decode, encode, waveform_cache_key, WaveformCachePort, WaveformIdentity,
    WAVEFORM_ALGORITHM_VERSION, WAVEFORM_FORMAT_VERSION,
};
use pulseseek_domain::waveform::levels::{Level, LevelIndex, MultiresolutionWaveform};
use pulseseek_domain::waveform::peak::Peak;

fn cache_path(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("app-cache.sqlite")
}

fn start_cache(dir: &tempfile::TempDir) -> TechnicalCache {
    TechnicalCache::start(cache_path(dir)).expect("cache starts")
}

fn identity(path: &str, size: u64, modified_ms: u64) -> WaveformIdentity {
    WaveformIdentity::new(Path::new(path), size, modified_ms)
}

fn open_raw(dir: &tempfile::TempDir) -> OpenedDatabase {
    open_or_recover(&cache_path(dir), &CACHE_MIGRATIONS).expect("open cache database")
}

fn waveform_row_count(dir: &tempfile::TempDir) -> usize {
    let mut database = open_raw(dir);
    let count: i64 = database
        .connection()
        .query_row("SELECT COUNT(*) FROM waveform_cache", [], |row| row.get(0))
        .expect("count waveform rows");
    count as usize
}

/// A stereo, two-level pyramid: coarsest 1 bucket, finest 2 buckets.
fn sample_waveform() -> MultiresolutionWaveform {
    let coarsest = Level {
        index: LevelIndex::new(0).expect("level 0"),
        samples_per_peak: 4,
        peaks: vec![Peak::from_parts(-0.5, 0.5), Peak::from_parts(-0.25, 0.75)],
    };
    let finest = Level {
        index: LevelIndex::new(1).expect("level 1"),
        samples_per_peak: 2,
        peaks: vec![
            Peak::from_parts(-0.4, 0.4),
            Peak::from_parts(-0.3, 0.6),
            Peak::from_parts(-0.2, 0.8),
            Peak::from_parts(-0.1, 0.9),
        ],
    };
    MultiresolutionWaveform::from_levels(2, vec![coarsest, finest]).expect("valid waveform")
}

// ── Hit and miss ──────────────────────────────────────────────────

#[test]
fn waveform_round_trips_through_cache() {
    let dir = tempfile::tempdir().unwrap();
    let cache = start_cache(&dir);
    let source = identity("/music/track.wav", 1000, 100);
    let key = waveform_cache_key(&source);

    cache.store_waveform(&key, &source, &sample_waveform()).expect("store");

    assert_eq!(
        cache.load_waveform(&key, &source).expect("load"),
        Some(sample_waveform()),
        "stored waveform loads as a hit"
    );
}

#[test]
fn missing_key_is_a_miss() {
    let dir = tempfile::tempdir().unwrap();
    let cache = start_cache(&dir);
    let source = identity("/music/other.wav", 1000, 100);
    let key = waveform_cache_key(&source);

    assert_eq!(cache.load_waveform(&key, &source).expect("load"), None);
}

// ── Stale validation ──────────────────────────────────────────────

#[test]
fn changed_modified_timestamp_invalidates_row() {
    let dir = tempfile::tempdir().unwrap();
    let cache = start_cache(&dir);
    let source = identity("/music/track.wav", 1000, 100);
    let key = waveform_cache_key(&source);
    cache.store_waveform(&key, &source, &sample_waveform()).expect("store");

    let changed = identity("/music/track.wav", 1000, 200);
    assert_eq!(
        cache.load_waveform(&key, &changed).expect("load"),
        None,
        "touched file must not reuse cached waveform"
    );
    drop(cache);

    assert_eq!(waveform_row_count(&dir), 0, "stale row must be deleted");
}

#[test]
fn changed_source_size_invalidates_row() {
    let dir = tempfile::tempdir().unwrap();
    let cache = start_cache(&dir);
    let source = identity("/music/track.wav", 1000, 100);
    let key = waveform_cache_key(&source);
    cache.store_waveform(&key, &source, &sample_waveform()).expect("store");

    let changed = identity("/music/track.wav", 999, 100);
    assert_eq!(
        cache.load_waveform(&key, &changed).expect("load"),
        None,
        "resized file must not reuse cached waveform"
    );
    drop(cache);

    assert_eq!(waveform_row_count(&dir), 0, "stale row must be deleted");
}

#[test]
fn changed_algorithm_version_invalidates_row() {
    let dir = tempfile::tempdir().unwrap();
    let cache = start_cache(&dir);
    let source = identity("/music/track.wav", 1000, 100);
    let key = waveform_cache_key(&source);
    cache.store_waveform(&key, &source, &sample_waveform()).expect("store");
    drop(cache);

    let mut database = open_raw(&dir);
    database
        .connection()
        .execute(
            "UPDATE waveform_cache SET algorithm_version = ?1 WHERE cache_key = ?2",
            rusqlite::params![999u32, key],
        )
        .expect("tamper algorithm version");
    drop(database);

    let reopened = start_cache(&dir);
    assert_eq!(
        reopened.load_waveform(&key, &source).expect("load"),
        None,
        "older algorithm data must not be reused"
    );
    drop(reopened);

    assert_eq!(waveform_row_count(&dir), 0, "stale row must be deleted");
}

// ── Corrupt cache row ─────────────────────────────────────────────

#[test]
fn corrupt_cache_row_is_miss_and_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let cache = start_cache(&dir);
    let source = identity("/music/track.wav", 1000, 100);
    let key = waveform_cache_key(&source);
    cache.store_waveform(&key, &source, &sample_waveform()).expect("store");
    drop(cache);

    let mut database = open_raw(&dir);
    database
        .connection()
        .execute(
            "UPDATE waveform_cache SET data = ?1 WHERE cache_key = ?2",
            rusqlite::params![b"not a waveform".to_vec(), key],
        )
        .expect("corrupt data blob");
    drop(database);

    let reopened = start_cache(&dir);
    assert_eq!(
        reopened.load_waveform(&key, &source).expect("load"),
        None,
        "undecodable row is a miss"
    );
    drop(reopened);

    assert_eq!(waveform_row_count(&dir), 0, "corrupt row must be deleted");
}

// ── Versioned file cache key ──────────────────────────────────────

#[test]
fn cache_key_is_deterministic_path_scoped_and_versioned() {
    let source = identity("/music/track.wav", 1000, 100);
    let key = waveform_cache_key(&source);

    assert!(key.starts_with("waveform:v1:"), "key carries a format version");
    assert_eq!(key, waveform_cache_key(&source), "same identity yields same key");
    assert_eq!(
        key,
        waveform_cache_key(&identity("/music/track.wav", 999, 50)),
        "size and mtime validate rows, they are not key material"
    );
    assert_ne!(
        key,
        waveform_cache_key(&identity("/music/other.wav", 1000, 100)),
        "path scopes the key"
    );
}

// ── Persistence and migration ─────────────────────────────────────

#[test]
fn waveform_cache_survives_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let cache = start_cache(&dir);
    let source = identity("/music/track.wav", 1000, 100);
    let key = waveform_cache_key(&source);
    cache.store_waveform(&key, &source, &sample_waveform()).expect("store");
    drop(cache);

    let reopened = start_cache(&dir);
    assert_eq!(
        reopened.load_waveform(&key, &source).expect("load"),
        Some(sample_waveform()),
        "cached waveform survives a restart"
    );
}

#[test]
fn v1_to_v2_migration_preserves_meta_and_adds_waveform_table() {
    let dir = tempfile::tempdir().unwrap();
    let path = cache_path(&dir);

    let mut v1 = open_or_recover(&path, &CACHE_MIGRATIONS[..1]).expect("open at schema v1");
    v1.connection()
        .execute(
            "INSERT INTO cache_meta (key, value, updated_at_ms) VALUES ('theme', ?1, 1)",
            rusqlite::params![b"midnight".to_vec()],
        )
        .expect("seed v1 metadata");
    drop(v1);

    let cache = TechnicalCache::start(&path).expect("upgrade to schema v2");
    assert_eq!(cache.status(), CacheStatus::Healthy { schema_version: 2 });

    let source = identity("/music/track.wav", 1000, 100);
    let key = waveform_cache_key(&source);
    cache.store_waveform(&key, &source, &sample_waveform()).expect("store");
    assert_eq!(
        cache.load_waveform(&key, &source).expect("load"),
        Some(sample_waveform()),
        "waveform table works after v1 -> v2 upgrade"
    );
    drop(cache);
}

// ── Acceptance: cache records never create manager items ──────────

#[test]
fn cache_operations_create_no_manager_database() {
    let dir = tempfile::tempdir().unwrap();
    let cache = start_cache(&dir);
    let source = identity("/music/track.wav", 1000, 100);
    let key = waveform_cache_key(&source);

    cache.store_waveform(&key, &source, &sample_waveform()).expect("store");
    let _ = cache.load_waveform(&key, &source).expect("load");
    drop(cache);

    let names: Vec<String> = fs::read_dir(dir.path())
        .expect("read dir")
        .map(|entry| entry.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    assert!(!names.iter().any(|n| n.contains("samples.sqlite")), "no samples manager database");
    assert!(!names.iter().any(|n| n.contains("music.sqlite")), "no music manager database");
    assert!(!names.iter().any(|n| n.contains("playlists.sqlite")), "no playlists manager database");
}

// ── Codec ─────────────────────────────────────────────────────────

#[test]
fn encode_round_trips_all_fields() {
    let waveform = sample_waveform();
    let bytes = encode(&waveform).expect("encode");
    assert_eq!(decode(&bytes).expect("decode"), waveform);
}

#[test]
fn decode_rejects_corrupt_bytes() {
    let waveform = sample_waveform();
    let bytes = encode(&waveform).expect("encode");

    assert!(decode(b"").is_err(), "empty bytes must be rejected");
    assert!(decode(&bytes[..4]).is_err(), "truncated header must be rejected");
    assert!(decode(&bytes[..bytes.len() - 1]).is_err(), "truncated peaks must be rejected");

    let mut bad_magic = bytes.clone();
    bad_magic[..4].copy_from_slice(b"XXXX");
    assert!(decode(&bad_magic).is_err(), "wrong magic must be rejected");

    let mut wrong_version = bytes.clone();
    wrong_version[4] = wrong_version[4].wrapping_add(1);
    assert!(decode(&wrong_version).is_err(), "wrong format version must be rejected");
}

#[test]
fn decode_rejects_oversized_peak_count_without_allocating() {
    // A hostile blob advertises a peak count far beyond the bytes it carries.
    // Decoding must fail cleanly instead of attempting a huge allocation.
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"PSWF");
    bytes.extend_from_slice(&WAVEFORM_FORMAT_VERSION.to_le_bytes());
    bytes.extend_from_slice(&WAVEFORM_ALGORITHM_VERSION.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes()); // channels
    bytes.push(1); // level count
    bytes.extend_from_slice(&0u32.to_le_bytes()); // level index
    bytes.extend_from_slice(&1u64.to_le_bytes()); // samples per peak
    bytes.extend_from_slice(&u64::MAX.to_le_bytes()); // advertised peak count
    assert!(decode(&bytes).is_err(), "oversized peak count must be rejected");
}
