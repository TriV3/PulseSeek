use std::fmt;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::thread;

use pulseseek_domain::waveform::levels::MultiresolutionWaveform;
use rusqlite::params;

use crate::sqlite::{now_ms, open_or_recover, Migration, OpenedDatabase, SqliteError};
use crate::waveform_cache::{
    encode, load_waveform_db, store_waveform_db, WaveformCacheError, WaveformCachePort,
    WaveformIdentity,
};

/// Schema version of the technical cache database.
pub const CACHE_SCHEMA_VERSION: u32 = 2;

/// Migrations for `app-cache.sqlite`.
///
/// The first version only carries cache metadata. Record tables (waveform,
/// recent folders, preferences, ...) are added by the feature PRs that own
/// them. Version 2 adds the waveform cache owned by PR-063.
pub const CACHE_MIGRATIONS: [Migration; 2] = [
    Migration {
        version: 1,
        sql: "CREATE TABLE cache_meta (\
              key TEXT PRIMARY KEY, \
              value BLOB NOT NULL, \
              updated_at_ms INTEGER NOT NULL);",
    },
    Migration {
        version: 2,
        sql: "CREATE TABLE waveform_cache (\
              cache_key TEXT PRIMARY KEY, \
              source_size INTEGER NOT NULL, \
              source_modified_ms INTEGER NOT NULL, \
              algorithm_version INTEGER NOT NULL, \
              data BLOB NOT NULL, \
              created_at_ms INTEGER NOT NULL);",
    },
];

/// Health of the technical cache database.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CacheStatus {
    /// The cache opened cleanly at the given schema version.
    Healthy { schema_version: u32 },
    /// The cache recovered from corruption and was recreated.
    Degraded { schema_version: u32, reason: String },
    /// The cache is not usable.
    Unavailable { reason: String },
}

/// Error produced by technical cache operations.
#[derive(Debug)]
pub enum CacheError {
    /// The database could not be opened, migrated, or backed up.
    Open(SqliteError),
    /// The dedicated cache worker could not be started.
    WorkerStart,
    /// The dedicated cache worker is no longer running.
    WorkerStopped,
    /// A query against the cache database failed.
    Sqlite(rusqlite::Error),
}

impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open(error) => write!(f, "technical cache unavailable: {error}"),
            Self::WorkerStart => write!(f, "cannot start technical cache worker"),
            Self::WorkerStopped => write!(f, "technical cache worker stopped"),
            Self::Sqlite(error) => write!(f, "technical cache query failed: {error}"),
        }
    }
}

impl std::error::Error for CacheError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Open(error) => Some(error),
            Self::Sqlite(error) => Some(error),
            _ => None,
        }
    }
}

/// Port used by application services to read and write technical cache data.
///
/// Implementations are backed by a dedicated worker so the SQLite connection
/// never blocks the UI or audio threads.
pub trait TechnicalCachePort: Send + Sync {
    /// Current health of the cache.
    fn status(&self) -> CacheStatus;

    /// Reads one metadata value, or `None` when the key is absent.
    fn get_meta(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError>;

    /// Writes one metadata value, replacing any existing value.
    fn set_meta(&self, key: &str, value: Vec<u8>) -> Result<(), CacheError>;

    /// Deletes one metadata value.
    fn remove_meta(&self, key: &str) -> Result<(), CacheError>;
}

enum WorkerCommand {
    Get {
        key: String,
        reply: SyncSender<Result<Option<Vec<u8>>, CacheError>>,
    },
    Set {
        key: String,
        value: Vec<u8>,
        reply: SyncSender<Result<(), CacheError>>,
    },
    Remove {
        key: String,
        reply: SyncSender<Result<(), CacheError>>,
    },
    StoreWaveform {
        key: String,
        identity: WaveformIdentity,
        waveform: MultiresolutionWaveform,
        reply: SyncSender<Result<(), WaveformCacheError>>,
    },
    LoadWaveform {
        key: String,
        identity: WaveformIdentity,
        reply: SyncSender<Result<Option<MultiresolutionWaveform>, WaveformCacheError>>,
    },
}

/// Client handle to the dedicated technical cache worker.
///
/// Opening and migrating the database happens on `start`; every subsequent
/// operation runs on the worker thread. The worker exits when this handle is
/// dropped and the command channel disconnects.
pub struct TechnicalCache {
    commands: Sender<WorkerCommand>,
    status: CacheStatus,
}

impl TechnicalCache {
    /// Opens `app-cache.sqlite` at `path` and starts its dedicated worker.
    ///
    /// Corruption is recovered by quarantining the corrupt file and recreating
    /// the schema; the cache then reports [`CacheStatus::Degraded`]. Any other
    /// open failure returns `Err` so the caller can keep running without the
    /// cache.
    pub fn start(path: impl AsRef<Path>) -> Result<Self, CacheError> {
        let database =
            open_or_recover(path.as_ref(), &CACHE_MIGRATIONS).map_err(CacheError::Open)?;
        let schema_version = database.schema_version();
        let status = if database.recovered() {
            CacheStatus::Degraded {
                schema_version,
                reason: "recovered from corrupt database".to_string(),
            }
        } else {
            CacheStatus::Healthy { schema_version }
        };

        let (commands, receiver) = mpsc::channel();
        thread::Builder::new()
            .name("pulseseek-cache".to_string())
            .spawn(move || worker_loop(receiver, database))
            .map_err(|_| CacheError::WorkerStart)?;

        Ok(Self { commands, status })
    }
}

impl TechnicalCachePort for TechnicalCache {
    fn status(&self) -> CacheStatus {
        self.status.clone()
    }

    fn get_meta(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.commands
            .send(WorkerCommand::Get { key: key.to_string(), reply: reply_tx })
            .map_err(|_| CacheError::WorkerStopped)?;
        reply_rx.recv().map_err(|_| CacheError::WorkerStopped)?
    }

    fn set_meta(&self, key: &str, value: Vec<u8>) -> Result<(), CacheError> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.commands
            .send(WorkerCommand::Set { key: key.to_string(), value, reply: reply_tx })
            .map_err(|_| CacheError::WorkerStopped)?;
        reply_rx.recv().map_err(|_| CacheError::WorkerStopped)?
    }

    fn remove_meta(&self, key: &str) -> Result<(), CacheError> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.commands
            .send(WorkerCommand::Remove { key: key.to_string(), reply: reply_tx })
            .map_err(|_| CacheError::WorkerStopped)?;
        reply_rx.recv().map_err(|_| CacheError::WorkerStopped)?
    }
}

impl WaveformCachePort for TechnicalCache {
    fn store_waveform(
        &self,
        key: &str,
        identity: &WaveformIdentity,
        waveform: &MultiresolutionWaveform,
    ) -> Result<(), WaveformCacheError> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.commands
            .send(WorkerCommand::StoreWaveform {
                key: key.to_string(),
                identity: identity.clone(),
                waveform: waveform.clone(),
                reply: reply_tx,
            })
            .map_err(|_| WaveformCacheError::WorkerStopped)?;
        reply_rx.recv().map_err(|_| WaveformCacheError::WorkerStopped)?
    }

    fn load_waveform(
        &self,
        key: &str,
        identity: &WaveformIdentity,
    ) -> Result<Option<MultiresolutionWaveform>, WaveformCacheError> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.commands
            .send(WorkerCommand::LoadWaveform {
                key: key.to_string(),
                identity: identity.clone(),
                reply: reply_tx,
            })
            .map_err(|_| WaveformCacheError::WorkerStopped)?;
        reply_rx.recv().map_err(|_| WaveformCacheError::WorkerStopped)?
    }
}

fn worker_loop(receiver: Receiver<WorkerCommand>, mut database: OpenedDatabase) {
    while let Ok(command) = receiver.recv() {
        match command {
            WorkerCommand::Get { key, reply } => {
                let _ = reply.send(get_meta(&mut database, &key));
            },
            WorkerCommand::Set { key, value, reply } => {
                let _ = reply.send(set_meta(&mut database, &key, value));
            },
            WorkerCommand::Remove { key, reply } => {
                let _ = reply.send(remove_meta(&mut database, &key));
            },
            WorkerCommand::StoreWaveform { key, identity, waveform, reply } => {
                let result = encode(&waveform)
                    .and_then(|data| store_waveform_db(&mut database, &key, &identity, &data));
                let _ = reply.send(result);
            },
            WorkerCommand::LoadWaveform { key, identity, reply } => {
                let _ = reply.send(load_waveform_db(&mut database, &key, &identity));
            },
        }
    }
}

fn get_meta(database: &mut OpenedDatabase, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
    let result = database.connection().query_row(
        "SELECT value FROM cache_meta WHERE key = ?1",
        params![key],
        |row| row.get::<_, Vec<u8>>(0),
    );
    match result {
        Ok(value) => Ok(Some(value)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(CacheError::Sqlite(error)),
    }
}

fn set_meta(database: &mut OpenedDatabase, key: &str, value: Vec<u8>) -> Result<(), CacheError> {
    database
        .connection()
        .execute(
            "INSERT INTO cache_meta (key, value, updated_at_ms) VALUES (?1, ?2, ?3) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, \
             updated_at_ms = excluded.updated_at_ms",
            params![key, value, now_ms()],
        )
        .map_err(CacheError::Sqlite)?;
    Ok(())
}

fn remove_meta(database: &mut OpenedDatabase, key: &str) -> Result<(), CacheError> {
    database
        .connection()
        .execute("DELETE FROM cache_meta WHERE key = ?1", params![key])
        .map_err(CacheError::Sqlite)?;
    Ok(())
}
