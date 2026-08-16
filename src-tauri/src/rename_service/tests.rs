use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use pulseseek_browser_fs::rename::NativeFileRename;
use pulseseek_cache::waveform_cache::{
    waveform_cache_key, WaveformCacheError, WaveformCachePort, WaveformIdentity,
};
use pulseseek_domain::browser::rename::{FileRename, RenameError};
use pulseseek_domain::error::{DiagnosticCode, DiagnosticContext, ErrorCategory, ErrorContract};
use pulseseek_domain::waveform::levels::MultiresolutionWaveform;
use tempfile::tempdir;

use super::*;

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
struct FakeFileRename {
    f: Box<dyn Fn(&Path, &str) -> Result<PathBuf, RenameError> + Send>,
}

impl FakeFileRename {
    #[allow(clippy::type_complexity)]
    fn new(f: Box<dyn Fn(&Path, &str) -> Result<PathBuf, RenameError> + Send>) -> Self {
        Self { f }
    }
}

impl FileRename for FakeFileRename {
    fn rename(&self, path: &Path, new_name: &str) -> Result<PathBuf, RenameError> {
        (self.f)(path, new_name)
    }
}

#[test]
fn native_rename_service_moves_file_and_returns_outcome() {
    let dir = tempdir().expect("tempdir");
    let source = dir.path().join("track.wav");
    std::fs::write(&source, "content").expect("write test file");

    let service = NativeRenameService::new(NativeFileRename, None);
    let outcome =
        service.rename(&source.to_string_lossy(), "renamed.wav").expect("rename succeeds");

    assert_eq!(outcome.old_path, source.to_string_lossy());
    assert_eq!(outcome.new_path, dir.path().join("renamed.wav").to_string_lossy());
    assert!(!source.exists(), "old path should be gone");
    assert!(Path::new(&outcome.new_path).exists(), "new path should exist");
}

#[test]
fn native_rename_service_maps_collision_to_conflict() {
    let rename = FakeFileRename::new(Box::new(|_, _| Err(RenameError::collision())));
    let service = NativeRenameService::new(rename, None);

    let error = service.rename("/music/track.wav", "existing.wav").expect_err("collision fails");
    assert_eq!(error.user_descriptor().category(), ErrorCategory::Conflict);
    assert_eq!(error.diagnostic_context().code(), DiagnosticCode::FileOperation.as_str());
}

#[test]
fn native_rename_service_maps_invalid_name_to_invalid_input() {
    let rename = FakeFileRename::new(Box::new(|_, _| Err(RenameError::invalid_name("empty"))));
    let service = NativeRenameService::new(rename, None);

    let error = service.rename("/music/track.wav", "").expect_err("invalid name fails");
    assert_eq!(error.user_descriptor().category(), ErrorCategory::InvalidInput);
}

#[test]
fn native_rename_service_invalidates_old_waveform_key() {
    let dir = tempdir().expect("tempdir");
    let source = dir.path().join("track.wav");
    std::fs::write(&source, "content").expect("write test file");
    let deleted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let cache: Arc<dyn WaveformCachePort> = Arc::new(RecordingCache::new(Arc::clone(&deleted)));
    let service = NativeRenameService::new(NativeFileRename, Some(cache));

    service.rename(&source.to_string_lossy(), "renamed.wav").expect("rename succeeds");

    let expected = waveform_cache_key(&WaveformIdentity::new(&source, 0, 0));
    let deleted_keys = deleted.lock().expect("lock");
    assert!(
        deleted_keys.iter().any(|key| key == &expected),
        "old waveform key should be invalidated; deleted keys: {deleted_keys:?}"
    );
}

#[test]
fn native_rename_service_cache_failure_does_not_block_rename() {
    let dir = tempdir().expect("tempdir");
    let source = dir.path().join("track.wav");
    std::fs::write(&source, "content").expect("write test file");
    let cache: Arc<dyn WaveformCachePort> = Arc::new(FailingCache);
    let service = NativeRenameService::new(NativeFileRename, Some(cache));

    let outcome = service
        .rename(&source.to_string_lossy(), "renamed.wav")
        .expect("rename succeeds despite cache failure");
    assert!(Path::new(&outcome.new_path).exists());
}

#[test]
fn native_rename_service_without_cache_succeeds() {
    let dir = tempdir().expect("tempdir");
    let source = dir.path().join("track.wav");
    std::fs::write(&source, "content").expect("write test file");
    let service = NativeRenameService::new(NativeFileRename, None);

    let outcome = service
        .rename(&source.to_string_lossy(), "renamed.wav")
        .expect("rename succeeds without cache");
    assert!(Path::new(&outcome.new_path).exists());
}

#[test]
fn fake_rename_service_forwards_call_and_error() {
    let service = FakeRenameService::new(Box::new(|path, name| {
        assert_eq!(path, "/music/track.wav");
        assert_eq!(name, "renamed.wav");
        Ok(RenameOutcome { old_path: path.to_string(), new_path: "/music/renamed.wav".to_string() })
    }));

    let outcome = service.rename("/music/track.wav", "renamed.wav").expect("fake succeeds");
    assert_eq!(outcome.new_path, "/music/renamed.wav");

    let failing = FakeRenameService::new(Box::new(|_, _| {
        Err(ApplicationError::new(
            ErrorCategory::Unavailable,
            DiagnosticContext::new(DiagnosticCode::FileOperation),
            std::io::Error::other("fake failure"),
        ))
    }));
    let error = failing.rename("/music/track.wav", "renamed.wav").expect_err("fake fails");
    assert_eq!(error.user_descriptor().category(), ErrorCategory::Unavailable);
}
