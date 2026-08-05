use std::sync::Arc;
use std::time::{Duration, Instant};

use pulseseek_cache::technical_cache::TechnicalCache;
use pulseseek_cache::waveform_cache::{waveform_cache_key, WaveformCachePort, WaveformIdentity};
use pulseseek_domain::error::ErrorContract;
use pulseseek_domain::waveform::levels::{Level, LevelIndex, MultiresolutionWaveform};
use pulseseek_domain::waveform::peak::Peak;

use super::*;
use crate::playback_events::{
    FakeEventEmitter, FileChangePayload, PlaybackEventEmitter, EVENT_FILE_CHANGE,
};

fn wait_for(timeout: Duration, what: &str, check: impl Fn() -> bool) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if check() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for {what}");
}

fn file_change_payloads(events: &Arc<FakeEventEmitter>) -> Vec<FileChangePayload> {
    events
        .recorded_events()
        .iter()
        .filter(|envelope| envelope.event == EVENT_FILE_CHANGE)
        .map(|envelope| {
            serde_json::from_value(envelope.payload.clone()).expect("file change payload")
        })
        .collect()
}

fn tiny_waveform() -> MultiresolutionWaveform {
    let level = Level {
        index: LevelIndex::new(0).expect("level 0"),
        samples_per_peak: 1,
        peaks: vec![Peak::from_parts(-0.5, 0.5)],
    };
    MultiresolutionWaveform::from_levels(1, vec![level]).expect("valid waveform")
}

// ── Fake service ──────────────────────────────────────────────────

#[test]
fn fake_watcher_records_calls() {
    let mut service = FakeFileWatcherService::new();
    assert_eq!(service.watched_path(), None);

    service.start_watching("/music").expect("start watching");
    assert_eq!(service.watch_calls, vec!["/music".to_string()]);
    assert_eq!(service.watched_path(), Some("/music".to_string()));

    service.stop_watching().expect("stop watching");
    assert_eq!(service.stop_calls, 1);
    assert_eq!(service.watched_path(), None);
}

#[test]
fn fake_watcher_reports_failure() {
    let mut service = FakeFileWatcherService::new();
    service.fail_watch = true;

    let error = service.start_watching("/music").expect_err("watch must fail");
    assert_eq!(error.user_descriptor().category(), ErrorCategory::Unavailable);
}

// ── Native service: change detection ──────────────────────────────

#[test]
fn native_watcher_emits_refresh_on_create() {
    let dir = tempfile::tempdir().expect("temp dir");
    let events = Arc::new(FakeEventEmitter::new());
    let mut service =
        NativeFileWatcherService::new(events.clone() as Arc<dyn PlaybackEventEmitter>, None, 100)
            .expect("watcher starts");
    service.start_watching(&dir.path().to_string_lossy()).expect("watch folder");

    std::fs::write(dir.path().join("new.wav"), b"data").expect("write file");
    wait_for(Duration::from_secs(5), "file change event", || {
        events.recorded_events().iter().any(|envelope| envelope.event == EVENT_FILE_CHANGE)
    });

    let payloads = file_change_payloads(&events);
    assert_eq!(payloads[0].path, dir.path().to_string_lossy(), "event carries watched folder");
}

#[test]
fn native_watcher_does_not_restart_same_folder() {
    let dir = tempfile::tempdir().expect("temp dir");
    let events = Arc::new(FakeEventEmitter::new());
    let mut service =
        NativeFileWatcherService::new(events.clone() as Arc<dyn PlaybackEventEmitter>, None, 100)
            .expect("watcher starts");
    let path = dir.path().to_string_lossy().to_string();

    service.start_watching(&path).expect("first watch");
    service.start_watching(&path).expect("same watch is a no-op");

    std::fs::write(dir.path().join("new.wav"), b"data").expect("write file");
    wait_for(Duration::from_secs(5), "file change event", || {
        events.recorded_events().iter().any(|envelope| envelope.event == EVENT_FILE_CHANGE)
    });

    assert_eq!(file_change_payloads(&events)[0].path, path);
}

#[test]
fn native_watcher_emits_refresh_on_delete() {
    let dir = tempfile::tempdir().expect("temp dir");
    let file = dir.path().join("bye.wav");
    std::fs::write(&file, b"data").expect("write file");
    let events = Arc::new(FakeEventEmitter::new());
    let mut service =
        NativeFileWatcherService::new(events.clone() as Arc<dyn PlaybackEventEmitter>, None, 100)
            .expect("watcher starts");
    service.start_watching(&dir.path().to_string_lossy()).expect("watch folder");

    std::fs::remove_file(&file).expect("remove file");
    wait_for(Duration::from_secs(5), "file change event", || {
        events.recorded_events().iter().any(|envelope| envelope.event == EVENT_FILE_CHANGE)
    });

    assert!(!file_change_payloads(&events).is_empty());
}

#[test]
fn native_watcher_emits_refresh_on_rename() {
    let dir = tempfile::tempdir().expect("temp dir");
    let from = dir.path().join("old.wav");
    let to = dir.path().join("new.wav");
    std::fs::write(&from, b"data").expect("write file");
    let events = Arc::new(FakeEventEmitter::new());
    let mut service =
        NativeFileWatcherService::new(events.clone() as Arc<dyn PlaybackEventEmitter>, None, 100)
            .expect("watcher starts");
    service.start_watching(&dir.path().to_string_lossy()).expect("watch folder");

    std::fs::rename(&from, &to).expect("rename file");
    wait_for(Duration::from_secs(5), "file change event", || {
        events.recorded_events().iter().any(|envelope| envelope.event == EVENT_FILE_CHANGE)
    });

    assert!(!file_change_payloads(&events).is_empty());
}

#[test]
fn native_watcher_invalidates_waveform_cache_on_modify() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cache_dir = tempfile::tempdir().expect("cache dir");
    let cache =
        TechnicalCache::start(cache_dir.path().join("app-cache.sqlite")).expect("cache starts");
    let file = dir.path().join("track.wav");
    std::fs::write(&file, b"v1").expect("write v1");

    let identity = WaveformIdentity::new(file.clone(), 2, 1);
    let key = waveform_cache_key(&identity);
    cache.store_waveform(&key, &identity, &tiny_waveform()).expect("seed cache");

    let events = Arc::new(FakeEventEmitter::new());
    let mut service = NativeFileWatcherService::new(
        events.clone() as Arc<dyn PlaybackEventEmitter>,
        Some(Arc::new(cache.clone())),
        100,
    )
    .expect("watcher starts");
    service.start_watching(&dir.path().to_string_lossy()).expect("watch folder");

    std::fs::write(&file, b"v2-longer").expect("modify file");
    wait_for(Duration::from_secs(5), "file change event", || {
        events.recorded_events().iter().any(|envelope| envelope.event == EVENT_FILE_CHANGE)
    });
    wait_for(Duration::from_secs(5), "cache invalidation", || {
        cache.load_waveform(&key, &identity).expect("load").is_none()
    });
}

#[test]
fn native_watcher_coalesces_bursts() {
    let dir = tempfile::tempdir().expect("temp dir");
    let events = Arc::new(FakeEventEmitter::new());
    let mut service =
        NativeFileWatcherService::new(events.clone() as Arc<dyn PlaybackEventEmitter>, None, 150)
            .expect("watcher starts");
    service.start_watching(&dir.path().to_string_lossy()).expect("watch folder");

    for index in 0..10 {
        std::fs::write(dir.path().join(format!("burst-{index}.wav")), b"data").expect("write file");
    }
    wait_for(Duration::from_secs(5), "file change event", || {
        events.recorded_events().iter().any(|envelope| envelope.event == EVENT_FILE_CHANGE)
    });

    let count = events
        .recorded_events()
        .iter()
        .filter(|envelope| envelope.event == EVENT_FILE_CHANGE)
        .count();
    assert!(count <= 3, "a burst of ten writes must coalesce into few refresh events, got {count}");
}

#[test]
fn native_watcher_stops_reporting_after_unwatch() {
    let dir = tempfile::tempdir().expect("temp dir");
    let events = Arc::new(FakeEventEmitter::new());
    let mut service =
        NativeFileWatcherService::new(events.clone() as Arc<dyn PlaybackEventEmitter>, None, 100)
            .expect("watcher starts");
    service.start_watching(&dir.path().to_string_lossy()).expect("watch folder");
    std::fs::write(dir.path().join("first.wav"), b"data").expect("write file");
    wait_for(Duration::from_secs(5), "first event", || {
        events.recorded_events().iter().any(|envelope| envelope.event == EVENT_FILE_CHANGE)
    });

    service.stop_watching().expect("stop watching");
    let before = events.recorded_events().len();
    std::fs::write(dir.path().join("second.wav"), b"data").expect("write file");
    std::thread::sleep(Duration::from_millis(400));

    assert_eq!(
        events.recorded_events().len(),
        before,
        "no refresh events after the watch is stopped"
    );
}

#[test]
fn native_watcher_rejects_missing_folder() {
    let dir = tempfile::tempdir().expect("temp dir");
    let events = Arc::new(FakeEventEmitter::new());
    let mut service =
        NativeFileWatcherService::new(events.clone() as Arc<dyn PlaybackEventEmitter>, None, 100)
            .expect("watcher starts");

    let missing = dir.path().join("removed-folder");
    let error = service
        .start_watching(&missing.to_string_lossy())
        .expect_err("missing folder must fail synchronously");
    assert_eq!(error.user_descriptor().category(), ErrorCategory::InvalidInput);
}

#[test]
fn on_debounced_invalidates_changed_sources() {
    use notify_debouncer_full::notify::event::{CreateKind, EventAttributes};
    use notify_debouncer_full::notify::{Event as NotifyEvent, EventKind};
    use notify_debouncer_full::{DebounceEventResult, DebouncedEvent};

    let dir = tempfile::tempdir().expect("temp dir");
    let cache_dir = tempfile::tempdir().expect("cache dir");
    let cache =
        TechnicalCache::start(cache_dir.path().join("app-cache.sqlite")).expect("cache starts");
    let file = dir.path().join("track.wav");
    let identity = WaveformIdentity::new(file.clone(), 2, 1);
    let key = waveform_cache_key(&identity);
    cache.store_waveform(&key, &identity, &tiny_waveform()).expect("seed cache");

    let events = Arc::new(FakeEventEmitter::new());
    let watched = std::sync::Mutex::new(Some(file.clone()));
    // macOS can report an overwrite as a create event.
    let notify_event = NotifyEvent {
        kind: EventKind::Create(CreateKind::File),
        paths: vec![file],
        attrs: EventAttributes::default(),
    };
    let result: DebounceEventResult =
        Ok(vec![DebouncedEvent { event: notify_event, time: Instant::now() }]);
    on_debounced(result, &watched, &*events, Some(&cache));

    assert!(
        cache.load_waveform(&key, &identity).expect("load").is_none(),
        "changed-source event must invalidate the cached waveform"
    );
    assert_eq!(events.event_count(), 1, "one refresh event is emitted");
}

#[test]
fn on_debounced_invalidates_modified_source() {
    use notify_debouncer_full::notify::event::{DataChange, EventAttributes, ModifyKind};
    use notify_debouncer_full::notify::{Event as NotifyEvent, EventKind};
    use notify_debouncer_full::{DebounceEventResult, DebouncedEvent};

    let dir = tempfile::tempdir().expect("temp dir");
    let cache_dir = tempfile::tempdir().expect("cache dir");
    let cache =
        TechnicalCache::start(cache_dir.path().join("app-cache.sqlite")).expect("cache starts");
    let file = dir.path().join("track.wav");
    let identity = WaveformIdentity::new(file.clone(), 2, 1);
    let key = waveform_cache_key(&identity);
    cache.store_waveform(&key, &identity, &tiny_waveform()).expect("seed cache");

    let events = Arc::new(FakeEventEmitter::new());
    let watched = std::sync::Mutex::new(Some(file.clone()));
    let notify_event = NotifyEvent {
        kind: EventKind::Modify(ModifyKind::Data(DataChange::Any)),
        paths: vec![file],
        attrs: EventAttributes::default(),
    };
    let result: DebounceEventResult =
        Ok(vec![DebouncedEvent { event: notify_event, time: Instant::now() }]);
    on_debounced(result, &watched, &*events, Some(&cache));

    assert!(
        cache.load_waveform(&key, &identity).expect("load").is_none(),
        "modify event must invalidate the cached waveform"
    );
    assert_eq!(events.event_count(), 1, "one refresh event is emitted");
}
