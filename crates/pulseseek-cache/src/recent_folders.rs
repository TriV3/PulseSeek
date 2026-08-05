//! Recent-folder history: persist and reopen a bounded MRU list of folders.
//!
//! Fulfills FR-BR-011. Recent folders are technical cache data in
//! `app-cache.sqlite`: a record is a plain path the user selected, never a
//! Sample Manager or Music Manager item. The store is path-agnostic — it
//! records a path without probing the filesystem, so a folder that disappears
//! later stays in the history and reopening it fails gracefully.
//!
//! # Privacy
//!
//! The store never logs paths. Failure messages produced by the surrounding
//! service layer use the application error contract, which only surfaces a
//! safe user message and never embeds the raw path. The stored `name` is the
//! basename only, so list payloads need not repeat a full path for display.

use std::fmt;
use std::path::PathBuf;

use rusqlite::params;

use crate::sqlite::{now_ms, OpenedDatabase};

/// Maximum number of recent folders retained. The oldest entry is evicted
/// when a record would exceed this bound.
pub const RECENT_FOLDERS_LIMIT: usize = 10;

/// One entry in the recent-folder history.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecentFolder {
    /// Filesystem path of the folder.
    pub path: String,
    /// Basename of the folder, used for display without repeating the path.
    pub name: String,
    /// Timestamp of the last record, in milliseconds since the Unix epoch.
    pub last_opened_ms: u64,
}

/// Error produced by recent-folder cache operations.
#[derive(Debug)]
pub enum RecentFoldersError {
    /// A query against the cache database failed.
    Sqlite(rusqlite::Error),
    /// The cache worker stopped before answering.
    WorkerStopped,
}

impl fmt::Display for RecentFoldersError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(error) => write!(f, "recent folder cache query failed: {error}"),
            Self::WorkerStopped => write!(f, "recent folder cache worker stopped"),
        }
    }
}

impl std::error::Error for RecentFoldersError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(error) => Some(error),
            Self::WorkerStopped => None,
        }
    }
}

/// Port for persisting the recent-folder history.
///
/// Implementations must be `Send` and `Sync` so the cache can live behind a
/// shared reference in application state.
pub trait RecentFoldersCachePort: Send + Sync {
    /// Records `path` as the most recently opened folder, moving it to the
    /// front when it already exists and evicting the oldest entry beyond
    /// [`RECENT_FOLDERS_LIMIT`].
    fn record_recent_folder(&self, path: &str) -> Result<(), RecentFoldersError>;

    /// Returns the recent folders ordered from most to least recent.
    fn list_recent_folders(&self) -> Result<Vec<RecentFolder>, RecentFoldersError>;

    /// Removes every recent-folder record.
    fn clear_recent_folders(&self) -> Result<(), RecentFoldersError>;
}

/// Records `path` as the most recently opened folder.
///
/// The opened timestamp is `now_ms` but never moves backwards: SQLite's scalar
/// `max` keeps it strictly larger than every existing row even when several
/// records land in the same millisecond, so ordering is deterministic. The
/// insert and the limit trim run in one transaction.
pub fn record_recent_folder_db(
    database: &mut OpenedDatabase,
    path: &str,
) -> Result<(), RecentFoldersError> {
    let name = folder_name(path);
    let transaction = database.connection().transaction().map_err(RecentFoldersError::Sqlite)?;

    transaction
        .execute(
            "INSERT INTO recent_folders (path, name, last_opened_ms) \
             VALUES (?1, ?2, max(?3, (SELECT coalesce(max(last_opened_ms), 0) + 1 \
             FROM recent_folders))) \
             ON CONFLICT(path) DO UPDATE SET \
             name = excluded.name, \
             last_opened_ms = excluded.last_opened_ms",
            params![path, name, now_ms()],
        )
        .map_err(RecentFoldersError::Sqlite)?;

    transaction
        .execute(
            "DELETE FROM recent_folders \
             WHERE path NOT IN (SELECT path FROM recent_folders \
             ORDER BY last_opened_ms DESC LIMIT ?1)",
            params![RECENT_FOLDERS_LIMIT as i64],
        )
        .map_err(RecentFoldersError::Sqlite)?;

    transaction.commit().map_err(RecentFoldersError::Sqlite)
}

/// Lists the recent folders from most to least recent.
pub fn list_recent_folders_db(
    database: &mut OpenedDatabase,
) -> Result<Vec<RecentFolder>, RecentFoldersError> {
    let mut statement = database
        .connection()
        .prepare(
            "SELECT path, name, last_opened_ms FROM recent_folders \
             ORDER BY last_opened_ms DESC",
        )
        .map_err(RecentFoldersError::Sqlite)?;
    let rows = statement
        .query_map([], |row| {
            Ok(RecentFolder {
                path: row.get(0)?,
                name: row.get(1)?,
                last_opened_ms: row.get::<_, i64>(2)? as u64,
            })
        })
        .map_err(RecentFoldersError::Sqlite)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(RecentFoldersError::Sqlite)
}

/// Removes every recent-folder record.
pub fn clear_recent_folders_db(database: &mut OpenedDatabase) -> Result<(), RecentFoldersError> {
    database
        .connection()
        .execute("DELETE FROM recent_folders", [])
        .map_err(RecentFoldersError::Sqlite)?;
    Ok(())
}

/// Derives the display name of a folder path.
///
/// Only the basename is used so the UI never needs to render a full path.
/// A root path keeps its trailing separator as its name.
fn folder_name(path: &str) -> String {
    let as_path = PathBuf::from(path);
    match as_path.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => path.to_string(),
    }
}
