//! Recent-folder history service (FR-BR-011).
//!
//! Application layer between the command envelope and the technical cache.
//! The native service persists through `pulseseek-cache`; when the cache is
//! unavailable the app falls back to an in-memory, session-only history so
//! recent folders keep working without blocking startup.
//!
//! # Privacy
//!
//! Recording validates the path through the shared path validator, whose
//! failures surface only the safe application error message. The service never
//! logs paths and never embeds a path in an error message; the stored display
//! name is the basename only.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use pulseseek_cache::recent_folders::{
    RecentFolder, RecentFoldersCachePort, RecentFoldersError, RECENT_FOLDERS_LIMIT,
};
use pulseseek_cache::technical_cache::{CacheError, TechnicalCachePort};
use pulseseek_domain::error::{ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory};

/// One recent-folder entry returned to the frontend.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct RecentFolderData {
    /// Filesystem path of the folder.
    pub path: String,
    /// Basename of the folder, used for display.
    pub name: String,
    /// Timestamp of the last record in milliseconds since the Unix epoch.
    pub last_opened_ms: u64,
}

const BOOKMARKS_CACHE_KEY: &str = "browser.bookmarks.v1";

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct FolderBookmarkData {
    pub path: String,
    pub name: String,
}

/// Port used by the command layer to read and update recent-folder history.
pub trait RecentFoldersService: Send + Sync {
    /// Returns the recent folders from most to least recent.
    fn list_recent_folders(&self) -> Result<Vec<RecentFolderData>, ApplicationError>;

    /// Records `path` as the most recently opened folder.
    ///
    /// Missing or non-directory paths fail with a safe `InvalidInput`-style
    /// error whose message never embeds the raw path. Virtual browser roots
    /// are ignored so they never pollute the history.
    fn record_recent_folder(&self, path: &str) -> Result<(), ApplicationError>;

    /// Removes every recent-folder record.
    fn clear_recent_folders(&self) -> Result<(), ApplicationError>;

    fn list_bookmarks(&self) -> Result<Vec<FolderBookmarkData>, ApplicationError>;
    fn add_bookmark(&self, path: &str) -> Result<(), ApplicationError>;
    fn remove_bookmark(&self, path: &str) -> Result<(), ApplicationError>;
}

/// Persists recent folders through the technical cache worker.
pub struct NativeRecentFoldersService {
    cache: Arc<dyn RecentFoldersCachePort>,
    meta_cache: Option<Arc<dyn TechnicalCachePort>>,
}

impl NativeRecentFoldersService {
    pub fn new(
        cache: Arc<dyn RecentFoldersCachePort>,
        meta_cache: Option<Arc<dyn TechnicalCachePort>>,
    ) -> Self {
        Self { cache, meta_cache }
    }

    fn store_bookmarks(&self, bookmarks: &[FolderBookmarkData]) -> Result<(), ApplicationError> {
        let Some(cache) = &self.meta_cache else { return Ok(()) };
        let value = serde_json::to_vec(bookmarks).map_err(bookmark_error_to_application)?;
        cache.set_meta(BOOKMARKS_CACHE_KEY, value).map_err(cache_meta_error_to_application)
    }
}

impl RecentFoldersService for NativeRecentFoldersService {
    fn list_recent_folders(&self) -> Result<Vec<RecentFolderData>, ApplicationError> {
        self.cache
            .list_recent_folders()
            .map(|folders| folders.into_iter().map(recent_folder_to_data).collect())
            .map_err(cache_error_to_application)
    }

    fn record_recent_folder(&self, path: &str) -> Result<(), ApplicationError> {
        if is_virtual_root(path) {
            return Ok(());
        }
        crate::path_validation::validate_directory(path)?;
        self.cache.record_recent_folder(path).map_err(cache_error_to_application)
    }

    fn clear_recent_folders(&self) -> Result<(), ApplicationError> {
        self.cache.clear_recent_folders().map_err(cache_error_to_application)
    }

    fn list_bookmarks(&self) -> Result<Vec<FolderBookmarkData>, ApplicationError> {
        let Some(cache) = &self.meta_cache else { return Ok(Vec::new()) };
        let Some(value) =
            cache.get_meta(BOOKMARKS_CACHE_KEY).map_err(cache_meta_error_to_application)?
        else {
            return Ok(Vec::new());
        };
        serde_json::from_slice(&value).map_err(bookmark_error_to_application)
    }

    fn add_bookmark(&self, path: &str) -> Result<(), ApplicationError> {
        validate_bookmark_path(path)?;
        let mut bookmarks = self.list_bookmarks()?;
        if !bookmarks.iter().any(|bookmark| bookmark.path == path) {
            bookmarks.push(FolderBookmarkData { path: path.to_string(), name: folder_name(path) });
            bookmarks.sort_by_key(|bookmark| bookmark.name.to_lowercase());
        }
        self.store_bookmarks(&bookmarks)
    }

    fn remove_bookmark(&self, path: &str) -> Result<(), ApplicationError> {
        let mut bookmarks = self.list_bookmarks()?;
        bookmarks.retain(|bookmark| bookmark.path != path);
        self.store_bookmarks(&bookmarks)
    }
}

/// Session-only recent-folder history used when the technical cache is
/// unavailable. It is also used as the fake in command-layer tests.
#[derive(Default)]
pub struct InMemoryRecentFoldersService {
    folders: Mutex<Vec<RecentFolderData>>,
    bookmarks: Mutex<Vec<FolderBookmarkData>>,
}

impl InMemoryRecentFoldersService {
    pub fn new() -> Self {
        Self::default()
    }
}

impl RecentFoldersService for InMemoryRecentFoldersService {
    fn list_recent_folders(&self) -> Result<Vec<RecentFolderData>, ApplicationError> {
        Ok(self.folders.lock().expect("recent folders lock poisoned").clone())
    }

    fn record_recent_folder(&self, path: &str) -> Result<(), ApplicationError> {
        if is_virtual_root(path) {
            return Ok(());
        }
        crate::path_validation::validate_directory(path)?;
        let mut folders = self.folders.lock().expect("recent folders lock poisoned");
        folders.retain(|folder| folder.path != path);
        let name = folder_name(path);
        let last_opened_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0);
        folders.insert(0, RecentFolderData { path: path.to_string(), name, last_opened_ms });
        folders.truncate(RECENT_FOLDERS_LIMIT);
        Ok(())
    }

    fn clear_recent_folders(&self) -> Result<(), ApplicationError> {
        self.folders.lock().expect("recent folders lock poisoned").clear();
        Ok(())
    }

    fn list_bookmarks(&self) -> Result<Vec<FolderBookmarkData>, ApplicationError> {
        Ok(self.bookmarks.lock().expect("bookmarks lock poisoned").clone())
    }

    fn add_bookmark(&self, path: &str) -> Result<(), ApplicationError> {
        validate_bookmark_path(path)?;
        let mut bookmarks = self.bookmarks.lock().expect("bookmarks lock poisoned");
        if !bookmarks.iter().any(|bookmark| bookmark.path == path) {
            bookmarks.push(FolderBookmarkData { path: path.to_string(), name: folder_name(path) });
            bookmarks.sort_by_key(|bookmark| bookmark.name.to_lowercase());
        }
        Ok(())
    }

    fn remove_bookmark(&self, path: &str) -> Result<(), ApplicationError> {
        self.bookmarks
            .lock()
            .expect("bookmarks lock poisoned")
            .retain(|bookmark| bookmark.path != path);
        Ok(())
    }
}

fn validate_bookmark_path(path: &str) -> Result<(), ApplicationError> {
    if is_virtual_root(path) {
        return Err(ApplicationError::new(
            ErrorCategory::InvalidInput,
            DiagnosticContext::new(DiagnosticCode::BrowserRead),
            std::io::Error::other("virtual browser roots cannot be bookmarked"),
        ));
    }
    crate::path_validation::validate_directory(path)
}

/// Rejects virtual browser roots so they never enter the history.
fn is_virtual_root(path: &str) -> bool {
    path == "computer://" || path.starts_with("computer://")
}

fn recent_folder_to_data(folder: RecentFolder) -> RecentFolderData {
    RecentFolderData { path: folder.path, name: folder.name, last_opened_ms: folder.last_opened_ms }
}

fn cache_error_to_application(error: RecentFoldersError) -> ApplicationError {
    ApplicationError::new(
        ErrorCategory::Unavailable,
        DiagnosticContext::new(DiagnosticCode::BrowserRead),
        error,
    )
}

fn cache_meta_error_to_application(error: CacheError) -> ApplicationError {
    ApplicationError::new(
        ErrorCategory::Unavailable,
        DiagnosticContext::new(DiagnosticCode::BrowserRead),
        error,
    )
}

fn bookmark_error_to_application(error: serde_json::Error) -> ApplicationError {
    ApplicationError::new(
        ErrorCategory::Unavailable,
        DiagnosticContext::new(DiagnosticCode::BrowserRead),
        error,
    )
}

/// Derives the display name of a folder path without repeating the full path.
fn folder_name(path: &str) -> String {
    match PathBuf::from(path).file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => path.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use pulseseek_domain::error::ErrorContract;

    fn record_existing_dir(
        service: &InMemoryRecentFoldersService,
        root: &tempfile::TempDir,
        name: &str,
    ) {
        let dir = root.path().join(name);
        fs::create_dir_all(&dir).expect("create folder");
        service.record_recent_folder(dir.to_str().expect("utf8 path")).expect("record");
    }

    #[test]
    fn missing_folder_record_fails_with_safe_message() {
        let service = InMemoryRecentFoldersService::new();
        let secret = "/nonexistent/secret-mix-folder";

        let error = service.record_recent_folder(secret).expect_err("missing folder fails");

        assert_eq!(error.user_descriptor().category(), ErrorCategory::InvalidInput);
        let safe_message = error.to_string();
        assert!(!safe_message.contains("secret-mix-folder"), "message must not embed path");
        assert!(!safe_message.contains("/nonexistent"), "message must not embed path");
    }

    #[test]
    fn virtual_root_is_never_recorded() {
        let service = InMemoryRecentFoldersService::new();

        service.record_recent_folder("computer://").expect("virtual root is ignored");

        assert!(service.list_recent_folders().expect("list").is_empty());
    }

    #[test]
    fn in_memory_service_reorders_and_bounds() {
        let service = InMemoryRecentFoldersService::new();
        let root = tempfile::tempdir().unwrap();
        for index in 0..(RECENT_FOLDERS_LIMIT + 3) {
            record_existing_dir(&service, &root, &format!("folder-{index:02}"));
        }
        let folders = service.list_recent_folders().expect("list");
        assert_eq!(folders.len(), RECENT_FOLDERS_LIMIT);
        assert_eq!(
            folders[0].path,
            root.path().join("folder-12").to_str().expect("utf8 path"),
            "newest folder is first"
        );
    }

    #[test]
    fn in_memory_service_clear_empties_history() {
        let service = InMemoryRecentFoldersService::new();
        let root = tempfile::tempdir().unwrap();
        record_existing_dir(&service, &root, "one");
        record_existing_dir(&service, &root, "two");
        service.clear_recent_folders().expect("clear");
        assert!(service.list_recent_folders().expect("list").is_empty());
    }

    #[test]
    fn bookmark_round_trip_is_deduplicated_and_removable() {
        let service = InMemoryRecentFoldersService::new();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().to_str().expect("utf8 path");

        service.add_bookmark(path).expect("add bookmark");
        service.add_bookmark(path).expect("duplicate is harmless");
        assert_eq!(service.list_bookmarks().expect("list").len(), 1);

        service.remove_bookmark(path).expect("remove bookmark");
        assert!(service.list_bookmarks().expect("list").is_empty());
    }

    #[test]
    fn virtual_root_cannot_be_bookmarked() {
        let service = InMemoryRecentFoldersService::new();
        assert!(service.add_bookmark("computer://").is_err());
    }

    #[test]
    fn native_service_maps_cache_failure_to_unavailable() {
        struct BrokenCache;
        impl RecentFoldersCachePort for BrokenCache {
            fn record_recent_folder(&self, _path: &str) -> Result<(), RecentFoldersError> {
                Err(RecentFoldersError::WorkerStopped)
            }
            fn list_recent_folders(&self) -> Result<Vec<RecentFolder>, RecentFoldersError> {
                Err(RecentFoldersError::WorkerStopped)
            }
            fn clear_recent_folders(&self) -> Result<(), RecentFoldersError> {
                Err(RecentFoldersError::WorkerStopped)
            }
        }

        let service = NativeRecentFoldersService::new(Arc::new(BrokenCache), None);
        let error = service.list_recent_folders().expect_err("broken cache fails");
        assert_eq!(error.user_descriptor().category(), ErrorCategory::Unavailable);
        assert_eq!(error.diagnostic_context().code(), DiagnosticCode::BrowserRead.as_str());
    }
}
