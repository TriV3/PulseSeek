use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use pulseseek_domain::browser::copy_file::{CopyError, CopyResult, FileCopy};
use pulseseek_domain::error::{ErrorCategory, ErrorContract};

use super::*;
use crate::playback_events::{CopyProgressPayload, FakeEventEmitter, EVENT_COPY_PROGRESS};

#[allow(clippy::type_complexity)]
struct FakeFileCopier {
    f: Box<dyn Fn(&[PathBuf], &Path, &AtomicBool) -> Vec<CopyResult> + Send + Sync>,
}

impl FakeFileCopier {
    #[allow(clippy::type_complexity)]
    fn new(
        f: Box<dyn Fn(&[PathBuf], &Path, &AtomicBool) -> Vec<CopyResult> + Send + Sync>,
    ) -> Self {
        Self { f }
    }
}

impl FileCopy for FakeFileCopier {
    fn copy_files(
        &self,
        files: &[PathBuf],
        target_dir: &Path,
        cancelled: &AtomicBool,
    ) -> Vec<CopyResult> {
        (self.f)(files, target_dir, cancelled)
    }
}

fn real_events() -> (Arc<FakeEventEmitter>, Arc<dyn PlaybackEventEmitter>) {
    let inner = Arc::new(FakeEventEmitter::new());
    let erased = inner.clone() as Arc<dyn PlaybackEventEmitter>;
    (inner, erased)
}

/// Polls the fake emitter until a `done` copy-progress event arrives or the
/// deadline passes, then returns the last copy-progress payload.
fn wait_for_done(events: &FakeEventEmitter) -> CopyProgressPayload {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        let recorded = events.recorded_events();
        let mut last: Option<CopyProgressPayload> = None;
        for envelope in &recorded {
            if envelope.event == EVENT_COPY_PROGRESS {
                let payload: CopyProgressPayload =
                    serde_json::from_value(envelope.payload.clone()).expect("payload parses");
                if payload.done {
                    return payload;
                }
                last = Some(payload);
            }
        }
        if std::time::Instant::now() > deadline {
            return last.expect("copy-progress done event never arrived");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn start_copy_rejects_missing_target_directory() {
    let copier = FakeFileCopier::new(Box::new(|_, _, _| vec![]));
    let service = NativeCopyService::new(copier);
    let (_, events) = real_events();
    let error = service
        .start_copy(vec!["/music/a.wav".to_string()], "/missing/dir".to_string(), events)
        .expect_err("missing target must fail");
    assert_eq!(error.user_descriptor().category(), ErrorCategory::InvalidInput);
}

#[test]
fn start_copy_rejects_empty_selection() {
    let dir = tempfile::tempdir().expect("tempdir");
    let copier = FakeFileCopier::new(Box::new(|_, _, _| vec![]));
    let service = NativeCopyService::new(copier);
    let (_, events) = real_events();
    let error = service
        .start_copy(vec![], dir.path().to_string_lossy().to_string(), events)
        .expect_err("empty selection must fail");
    assert_eq!(error.user_descriptor().category(), ErrorCategory::InvalidInput);
}

#[test]
fn start_copy_returns_session_id() {
    let dir = tempfile::tempdir().expect("tempdir");
    let copier = FakeFileCopier::new(Box::new(|_, _, _| vec![]));
    let service = NativeCopyService::new(copier);
    let (_, events) = real_events();
    let session_id = service
        .start_copy(
            vec!["/music/a.wav".to_string()],
            dir.path().to_string_lossy().to_string(),
            events,
        )
        .expect("start succeeds");
    assert!(session_id.starts_with("copy-"));
}

#[test]
fn copy_worker_emits_progress_events_per_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let copier = FakeFileCopier::new(Box::new(|files, target, _| {
        files
            .iter()
            .map(|path| {
                let name = path.file_name().expect("file name");
                (path.clone(), Ok(target.join(name)))
            })
            .collect()
    }));
    let service = NativeCopyService::new(copier);
    let (events, erased) = real_events();
    service
        .start_copy(
            vec!["/music/a.wav".to_string(), "/music/b.wav".to_string()],
            dir.path().to_string_lossy().to_string(),
            erased,
        )
        .expect("start succeeds");
    let final_payload = wait_for_done(&events);
    let recorded = events.recorded_events();
    let progress_events: Vec<_> =
        recorded.iter().filter(|envelope| envelope.event == EVENT_COPY_PROGRESS).collect();
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
        if envelope.event != EVENT_COPY_PROGRESS {
            continue;
        }
        let payload: CopyProgressPayload =
            serde_json::from_value(envelope.payload.clone()).expect("payload parses");
        if !payload.done {
            seen_intermediate = true;
            assert!(payload.results.is_empty(), "intermediate event must omit results");
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
fn copy_worker_reports_partial_failure_separately() {
    let dir = tempfile::tempdir().expect("tempdir");
    let copier = FakeFileCopier::new(Box::new(|files, target, _| {
        files
            .iter()
            .enumerate()
            .map(|(index, path)| {
                if index == 0 {
                    let name = path.file_name().expect("file name");
                    (path.clone(), Ok(target.join(name)))
                } else {
                    (path.clone(), Err(CopyError::collision()))
                }
            })
            .collect()
    }));
    let service = NativeCopyService::new(copier);
    let (events, erased) = real_events();
    service
        .start_copy(
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
fn cancel_copy_marks_remaining_files_cancelled() {
    let dir = tempfile::tempdir().expect("tempdir");
    let copier = FakeFileCopier::new(Box::new(|files, target, cancelled| {
        let mut results = Vec::new();
        for (index, path) in files.iter().enumerate() {
            if index == 0 {
                let name = path.file_name().expect("file name");
                results.push((path.clone(), Ok(target.join(name))));
                // Simulate the user cancelling after the first file.
                cancelled.store(true, Ordering::Release);
            } else {
                results.push((path.clone(), Err(CopyError::cancelled())));
            }
        }
        results
    }));
    let service = NativeCopyService::new(copier);
    let (events, erased) = real_events();
    let session_id = service
        .start_copy(
            vec!["/music/a.wav".to_string(), "/music/b.wav".to_string()],
            dir.path().to_string_lossy().to_string(),
            erased,
        )
        .expect("start succeeds");
    service.cancel_copy(&session_id);
    let final_payload = wait_for_done(&events);
    assert!(final_payload.done);
    assert_eq!(final_payload.completed, 2);
    assert!(final_payload.results[0].ok);
    assert!(!final_payload.results[1].ok, "cancelled file reported as failed");
    assert_eq!(final_payload.results[1].category.as_deref(), Some("Cancelled"));
}
