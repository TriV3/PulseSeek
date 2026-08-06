use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use pulseseek_cache::waveform_cache::{
    waveform_cache_key, WaveformCacheError, WaveformCachePort, WaveformIdentity,
};
use pulseseek_domain::browser::move_file::{FileMove, MoveError, MoveResult};
use pulseseek_domain::error::{ErrorCategory, ErrorContract};
use pulseseek_domain::waveform::levels::MultiresolutionWaveform;
use tempfile::tempdir;

use super::*;
use crate::playback_events::{FakeEventEmitter, MoveProgressPayload, EVENT_MOVE_PROGRESS};

/// Records every key passed to `delete_waveform`.
struct RecordingCache {
    deleted: Arc<Mutex<Vec<String>>>,
}

impl RecordingCache {
    fn new(deleted: Arc<Mutex<Vec<String>>>) -> Self {
        Self { deleted }
    }
}

impl WaveformCachePort for RecordingCache {
    fn store_waveform(
        &self,
        _key: &str,
        _identity: &WaveformIdentity,
        _waveform: &MultiresolutionWaveform,
    ) -> Result<(), WaveformCacheError> {
        Ok(())
    }

    fn load_waveform(
        &self,
        _key: &str,
        _identity: &WaveformIdentity,
    ) -> Result<Option<MultiresolutionWaveform>, WaveformCacheError> {
        Ok(None)
    }

    fn delete_waveform(&self, key: &str) -> Result<(), WaveformCacheError> {
        self.deleted.lock().expect("lock").push(key.to_string());
        Ok(())
    }
}

/// Cache whose `delete_waveform` always fails.
struct FailingCache;

impl WaveformCachePort for FailingCache {
    fn store_waveform(
        &self,
        _key: &str,
        _identity: &WaveformIdentity,
        _waveform: &MultiresolutionWaveform,
    ) -> Result<(), WaveformCacheError> {
        Ok(())
    }

    fn load_waveform(
        &self,
        _key: &str,
        _identity: &WaveformIdentity,
    ) -> Result<Option<MultiresolutionWaveform>, WaveformCacheError> {
        Ok(None)
    }

    fn delete_waveform(&self, _key: &str) -> Result<(), WaveformCacheError> {
        Err(WaveformCacheError::WorkerStopped)
    }
}

#[allow(clippy::type_complexity)]
struct FakeFileMover {
    f: Box<dyn Fn(&[PathBuf], &Path, &AtomicBool) -> Vec<MoveResult> + Send + Sync>,
}

impl FakeFileMover {
    #[allow(clippy::type_complexity)]
    fn new(
        f: Box<dyn Fn(&[PathBuf], &Path, &AtomicBool) -> Vec<MoveResult> + Send + Sync>,
    ) -> Self {
        Self { f }
    }
}

impl FileMove for FakeFileMover {
    fn move_files(
        &self,
        files: &[PathBuf],
        target_dir: &Path,
        cancelled: &AtomicBool,
    ) -> Vec<MoveResult> {
        (self.f)(files, target_dir, cancelled)
    }
}

fn real_events() -> (Arc<FakeEventEmitter>, Arc<dyn PlaybackEventEmitter>) {
    let inner = Arc::new(FakeEventEmitter::new());
    let erased = inner.clone() as Arc<dyn PlaybackEventEmitter>;
    (inner, erased)
}

/// Polls the fake emitter until a `done` move-progress event arrives or the
/// deadline passes, then returns the last move-progress payload.
fn wait_for_done(events: &FakeEventEmitter) -> MoveProgressPayload {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        let recorded = events.recorded_events();
        let mut last: Option<MoveProgressPayload> = None;
        for envelope in &recorded {
            if envelope.event == EVENT_MOVE_PROGRESS {
                let payload: MoveProgressPayload =
                    serde_json::from_value(envelope.payload.clone()).expect("payload parses");
                if payload.done {
                    return payload;
                }
                last = Some(payload);
            }
        }
        if std::time::Instant::now() > deadline {
            return last.expect("move-progress done event never arrived");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn start_move_rejects_missing_target_directory() {
    let mover = FakeFileMover::new(Box::new(|_, _, _| vec![]));
    let service = NativeMoveService::new(mover);
    let (_, events) = real_events();
    let error = service
        .start_move(vec!["/music/a.wav".to_string()], "/missing/dir".to_string(), events)
        .expect_err("missing target must fail");
    assert_eq!(error.user_descriptor().category(), ErrorCategory::InvalidInput);
}

#[test]
fn start_move_rejects_empty_selection() {
    let dir = tempdir().expect("tempdir");
    let mover = FakeFileMover::new(Box::new(|_, _, _| vec![]));
    let service = NativeMoveService::new(mover);
    let (_, events) = real_events();
    let error = service
        .start_move(vec![], dir.path().to_string_lossy().to_string(), events)
        .expect_err("empty selection must fail");
    assert_eq!(error.user_descriptor().category(), ErrorCategory::InvalidInput);
}

#[test]
fn start_move_returns_session_id() {
    let dir = tempdir().expect("tempdir");
    let mover = FakeFileMover::new(Box::new(|_, _, _| vec![]));
    let service = NativeMoveService::new(mover);
    let (_, events) = real_events();
    let session_id = service
        .start_move(
            vec!["/music/a.wav".to_string()],
            dir.path().to_string_lossy().to_string(),
            events,
        )
        .expect("start succeeds");
    assert!(session_id.starts_with("move-"));
}

#[test]
fn move_worker_emits_progress_events_per_file() {
    let dir = tempdir().expect("tempdir");
    let mover = FakeFileMover::new(Box::new(|files, target, _| {
        files
            .iter()
            .map(|path| {
                let name = path.file_name().expect("file name");
                (path.clone(), Ok(target.join(name)))
            })
            .collect()
    }));
    let service = NativeMoveService::new(mover);
    let (events, erased) = real_events();
    service
        .start_move(
            vec!["/music/a.wav".to_string(), "/music/b.wav".to_string()],
            dir.path().to_string_lossy().to_string(),
            erased,
        )
        .expect("start succeeds");

    let final_payload = wait_for_done(&events);
    let recorded = events.recorded_events();
    let progress_events: Vec<_> =
        recorded.iter().filter(|envelope| envelope.event == EVENT_MOVE_PROGRESS).collect();
    assert_eq!(progress_events.len(), 2, "one event per file");
    assert!(final_payload.done, "last event must be done");
    assert_eq!(final_payload.completed, 2);
    assert_eq!(final_payload.total, 2);
    assert_eq!(final_payload.results.len(), 2);
    // Intermediate events must not resend the cumulative results array
    // (keeps payloads O(1) per event for large batches); only the final
    // done event carries the full results.
    let mut seen_intermediate = false;
    for envelope in &recorded {
        if envelope.event != EVENT_MOVE_PROGRESS {
            continue;
        }
        let payload: MoveProgressPayload =
            serde_json::from_value(envelope.payload.clone()).expect("payload parses");
        if !payload.done {
            seen_intermediate = true;
            assert!(
                payload.results.is_empty(),
                "intermediate event must omit results"
            );
        }
    }
    assert!(seen_intermediate, "an intermediate event must exist");
    assert!(final_payload.results[0].ok);
    let expected_first = dir.path().join("a.wav").to_string_lossy().to_string();
    assert_eq!(final_payload.results[0].new_path.as_deref(), Some(expected_first.as_str()));
    assert!(final_payload.results[1].ok);
    let expected_second = dir.path().join("b.wav").to_string_lossy().to_string();
    assert_eq!(final_payload.results[1].new_path.as_deref(), Some(expected_second.as_str()));
}

#[test]
fn move_worker_reports_partial_failure_separately() {
    let dir = tempdir().expect("tempdir");
    let mover = FakeFileMover::new(Box::new(|files, target, _| {
        files
            .iter()
            .enumerate()
            .map(|(index, path)| {
                if index == 0 {
                    let name = path.file_name().expect("file name");
                    (path.clone(), Ok(target.join(name)))
                } else {
                    (path.clone(), Err(MoveError::collision()))
                }
            })
            .collect()
    }));
    let service = NativeMoveService::new(mover);
    let (events, erased) = real_events();
    service
        .start_move(
            vec!["/music/a.wav".to_string(), "/music/b.wav".to_string()],
            dir.path().to_string_lossy().to_string(),
            erased,
        )
        .expect("start succeeds");

    let final_payload = wait_for_done(&events);
    assert!(final_payload.done);
    assert_eq!(final_payload.results.len(), 2);
    assert!(final_payload.results[0].ok, "successful target reported as ok");
    let expected = dir.path().join("a.wav").to_string_lossy().to_string();
    assert_eq!(final_payload.results[0].new_path.as_deref(), Some(expected.as_str()));
    assert!(!final_payload.results[1].ok, "failed target reported separately");
    assert_eq!(final_payload.results[1].category.as_deref(), Some("Conflict"));
    assert!(final_payload.results[1].message.is_some(), "failed target carries a safe message");
    assert_eq!(final_payload.results[1].diagnostic_code.as_deref(), Some("file.operation"));
}

#[test]
fn cancel_move_marks_remaining_files_cancelled() {
    let dir = tempdir().expect("tempdir");
    let mover = FakeFileMover::new(Box::new(|files, target, cancelled| {
        let mut results = Vec::new();
        for (index, path) in files.iter().enumerate() {
            if index == 0 {
                let name = path.file_name().expect("file name");
                results.push((path.clone(), Ok(target.join(name))));
                // Simulate the user cancelling after the first file.
                cancelled.store(true, Ordering::Release);
            } else {
                results.push((path.clone(), Err(MoveError::cancelled())));
            }
        }
        results
    }));
    let service = NativeMoveService::new(mover);
    let (events, erased) = real_events();
    let session_id = service
        .start_move(
            vec!["/music/a.wav".to_string(), "/music/b.wav".to_string()],
            dir.path().to_string_lossy().to_string(),
            erased,
        )
        .expect("start succeeds");
    service.cancel_move(&session_id);

    let final_payload = wait_for_done(&events);
    assert!(final_payload.done);
    assert_eq!(final_payload.completed, 2);
    assert!(final_payload.results[0].ok);
    assert!(!final_payload.results[1].ok, "cancelled file reported as failed");
    assert_eq!(final_payload.results[1].category.as_deref(), Some("Cancelled"));
}

#[test]
fn move_worker_reconciles_moved_file_paths() {
    let dir = tempdir().expect("tempdir");
    let reconciled: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let reconciled_for_closure = Arc::clone(&reconciled);
    let mover = FakeFileMover::new(Box::new(|files, target, _| {
        files
            .iter()
            .map(|path| {
                let name = path.file_name().expect("file name");
                (path.clone(), Ok(target.join(name)))
            })
            .collect()
    }));
    let service = NativeMoveService::new(mover).with_reconcile(Some(Arc::new(
        move |old: &str, new: &str| {
            reconciled_for_closure.lock().expect("lock").push((old.to_string(), new.to_string()));
            Ok(false)
        },
    )));
    let (events, erased) = real_events();
    service
        .start_move(
            vec!["/music/a.wav".to_string()],
            dir.path().to_string_lossy().to_string(),
            erased,
        )
        .expect("start succeeds");

    let final_payload = wait_for_done(&events);
    assert!(final_payload.done);
    assert!(final_payload.results[0].ok, "move stays ok even when reconcile reports");
    let recorded = reconciled.lock().expect("lock");
    assert_eq!(recorded.len(), 1, "reconcile called once per moved file");
    assert_eq!(recorded[0].0, "/music/a.wav");
    assert_eq!(recorded[0].1, dir.path().join("a.wav").to_string_lossy());
}

#[test]
fn move_worker_invalidates_waveform_cache_for_old_paths() {
    let dir = tempdir().expect("tempdir");
    let deleted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let cache = RecordingCache::new(Arc::clone(&deleted));
    let mover = FakeFileMover::new(Box::new(|files, target, _| {
        files
            .iter()
            .map(|path| {
                let name = path.file_name().expect("file name");
                (path.clone(), Ok(target.join(name)))
            })
            .collect()
    }));
    let service = NativeMoveService::new(mover).with_cache(Some(Arc::new(cache)));
    let (events, erased) = real_events();
    service
        .start_move(
            vec!["/music/a.wav".to_string()],
            dir.path().to_string_lossy().to_string(),
            erased,
        )
        .expect("start succeeds");

    let final_payload = wait_for_done(&events);
    assert!(final_payload.done);
    let expected_key =
        waveform_cache_key(&WaveformIdentity::new(PathBuf::from("/music/a.wav"), 0, 0));
    let deleted = deleted.lock().expect("lock");
    assert!(
        deleted.iter().any(|key| key == &expected_key),
        "old-path cache row must be invalidated, got {deleted:?}"
    );
}

#[test]
fn move_worker_tolerates_failing_cache_and_reconcile() {
    let dir = tempdir().expect("tempdir");
    let mover = FakeFileMover::new(Box::new(|files, target, _| {
        files
            .iter()
            .map(|path| {
                let name = path.file_name().expect("file name");
                (path.clone(), Ok(target.join(name)))
            })
            .collect()
    }));
    let service = NativeMoveService::new(mover)
        .with_cache(Some(Arc::new(FailingCache)))
        .with_reconcile(Some(Arc::new(|_, _| {
            Err(ApplicationError::new(
                ErrorCategory::Internal,
                DiagnosticContext::new(DiagnosticCode::FileOperation),
                std::io::Error::other("reconcile failed"),
            ))
        })));
    let (events, erased) = real_events();
    service
        .start_move(
            vec!["/music/a.wav".to_string()],
            dir.path().to_string_lossy().to_string(),
            erased,
        )
        .expect("start succeeds");

    let final_payload = wait_for_done(&events);
    assert!(final_payload.done);
    assert!(final_payload.results[0].ok, "cache or reconcile failure must not fail the move");
}
