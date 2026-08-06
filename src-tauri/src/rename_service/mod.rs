//! Rename service (FR-FM-004, FR-FM-009, FR-FM-010).
//!
//! Application layer between the command envelope and the native filesystem
//! adapter. The native service renames one file through `pulseseek-browser-fs`
//! and, on success, proactively invalidates the cached waveform row derived
//! from the old path so the visible item and cache identity stay consistent
//! without waiting for the file watcher (FR-FM-010). A cache failure or an
//! unavailable cache never blocks the rename.
//!
//! # Privacy
//!
//! Rename errors surface only the safe application error message; neither the
//! old nor the new path is ever embedded in a user-facing message or logged.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use pulseseek_cache::waveform_cache::{waveform_cache_key, WaveformCachePort, WaveformIdentity};
use pulseseek_domain::browser::rename::FileRename;
use pulseseek_domain::error::{ApplicationError, ErrorContract};

/// Outcome of a successful rename, ready for the command layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenameOutcome {
    /// Path of the file before the rename.
    pub old_path: String,
    /// Full path of the file after the rename.
    pub new_path: String,
}

pub trait RenameService: Send {
    /// Renames `path` to `new_name` within the same directory.
    fn rename(&self, path: &str, new_name: &str) -> Result<RenameOutcome, ApplicationError>;
}

#[allow(clippy::type_complexity)]
pub struct NativeRenameService<T: FileRename> {
    inner: T,
    cache: Option<Arc<dyn WaveformCachePort>>,
}

impl<T: FileRename> NativeRenameService<T> {
    pub fn new(inner: T, cache: Option<Arc<dyn WaveformCachePort>>) -> Self {
        Self { inner, cache }
    }

    /// Proactively deletes waveform rows stored under the old path so the
    /// renamed file starts from a clean cache (FR-FM-010). The key scheme
    /// canonicalizes paths, so both the raw and canonicalized forms are
    /// invalidated, matching the file watcher's candidate logic.
    fn invalidate_cache(&self, old_path: &Path) {
        let Some(cache) = &self.cache else { return };
        for candidate in cache_key_candidates(old_path) {
            let key = waveform_cache_key(&WaveformIdentity::new(candidate, 0, 0));
            if let Err(error) = cache.delete_waveform(&key) {
                tracing::warn!(error = %error, "waveform cache invalidation failed");
            }
        }
    }
}

impl<T: FileRename> RenameService for NativeRenameService<T> {
    fn rename(&self, path: &str, new_name: &str) -> Result<RenameOutcome, ApplicationError> {
        let old_path = PathBuf::from(path);
        let new_path = self.inner.rename(&old_path, new_name).map_err(|error| {
            let category = error.category();
            let context = error.diagnostic_context();
            ApplicationError::new(category, context, error)
        })?;
        self.invalidate_cache(&old_path);
        Ok(RenameOutcome {
            old_path: old_path.to_string_lossy().to_string(),
            new_path: new_path.to_string_lossy().to_string(),
        })
    }
}

/// Path forms that may have produced the stored cache key.
///
/// The key scheme canonicalizes paths, so a removed file is matched through
/// its canonical parent directory even when the delivered path is gone. This
/// mirrors `file_watcher_service::native` so proactive invalidation stays
/// aligned with rows the watcher would have invalidated.
fn cache_key_candidates(path: &Path) -> Vec<PathBuf> {
    let mut candidates = vec![path.to_path_buf()];
    if let Ok(canonical) = std::fs::canonicalize(path) {
        candidates.push(canonical);
    } else if let (Some(parent), Some(name)) = (path.parent(), path.file_name()) {
        if let Ok(canonical_parent) = std::fs::canonicalize(parent) {
            candidates.push(canonical_parent.join(name));
        }
    }
    candidates
}

/// In-memory rename service used by command-layer tests.
#[allow(clippy::type_complexity)]
pub struct FakeRenameService {
    f: Box<dyn Fn(&str, &str) -> Result<RenameOutcome, ApplicationError> + Send>,
}

impl FakeRenameService {
    #[allow(clippy::type_complexity)]
    pub fn new(
        f: Box<dyn Fn(&str, &str) -> Result<RenameOutcome, ApplicationError> + Send>,
    ) -> Self {
        Self { f }
    }
}

impl RenameService for FakeRenameService {
    fn rename(&self, path: &str, new_name: &str) -> Result<RenameOutcome, ApplicationError> {
        (self.f)(path, new_name)
    }
}

#[cfg(test)]
mod tests;
