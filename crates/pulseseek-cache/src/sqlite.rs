use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};

/// A versioned SQL migration applied in a single transaction.
#[derive(Clone, Copy, Debug)]
pub struct Migration {
    /// Schema version produced by this migration.
    pub version: u32,
    /// SQL statements applied atomically.
    pub sql: &'static str,
}

/// Errors produced while opening or migrating a SQLite database.
#[derive(Debug)]
pub enum SqliteError {
    /// The database file could not be opened or read.
    Open { path: PathBuf, source: rusqlite::Error },
    /// The existing file is not a SQLite database.
    NotADatabase { path: PathBuf, source: rusqlite::Error },
    /// A migration failed and was rolled back.
    Migration { version: u32, source: rusqlite::Error },
    /// A pre-migration backup could not be written.
    Backup { path: PathBuf, source: std::io::Error },
    /// A filesystem operation (quarantine, directory creation) failed.
    Io { path: PathBuf, source: std::io::Error },
}

impl fmt::Display for SqliteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open { path, .. } => write!(f, "cannot open database {}", path.display()),
            Self::NotADatabase { path, .. } => {
                write!(f, "{} is not a sqlite database", path.display())
            },
            Self::Migration { version, .. } => {
                write!(f, "database migration to version {version} failed")
            },
            Self::Backup { path, .. } => write!(f, "cannot back up database to {}", path.display()),
            Self::Io { path, .. } => write!(f, "database filesystem error for {}", path.display()),
        }
    }
}

impl std::error::Error for SqliteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Open { source, .. }
            | Self::NotADatabase { source, .. }
            | Self::Migration { source, .. } => Some(source),
            Self::Backup { source, .. } | Self::Io { source, .. } => Some(source),
        }
    }
}

/// A migrated SQLite database owned by a single thread.
///
/// A `Connection` is not `Sync`, so the owning thread must be the only one
/// touching the database. The technical cache keeps its connection inside a
/// dedicated worker.
pub struct OpenedDatabase {
    connection: Connection,
    schema_version: u32,
    recovered: bool,
}

impl OpenedDatabase {
    /// Highest migration version applied.
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Whether the database was recreated after detecting corruption.
    pub fn recovered(&self) -> bool {
        self.recovered
    }

    /// Mutable access to the underlying connection.
    pub fn connection(&mut self) -> &mut Connection {
        &mut self.connection
    }
}

impl fmt::Debug for OpenedDatabase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenedDatabase")
            .field("schema_version", &self.schema_version)
            .field("recovered", &self.recovered)
            .finish_non_exhaustive()
    }
}

/// Opens a database, applies pending migrations, and recovers corruption.
///
/// - A fresh file is created and migrated from scratch.
/// - An up-to-date file is opened without changes (repeat migration is a
///   no-op and never creates a backup).
/// - A file that is not a database is quarantined and recreated; the returned
///   database reports `recovered() == true`.
/// - Before applying any migration to an existing file, the current file is
///   copied to `<name>.backup-<version>.sqlite`.
pub fn open_or_recover(
    path: &Path,
    migrations: &[Migration],
) -> Result<OpenedDatabase, SqliteError> {
    let mut recovered = false;
    let mut connection = match open_connection(path) {
        Ok(connection) => connection,
        Err(SqliteError::NotADatabase { .. }) => {
            quarantine(path)?;
            recovered = true;
            open_connection(path)?
        },
        Err(error) => return Err(error),
    };
    let schema_version = run_migrations(path, &mut connection, migrations)?;
    Ok(OpenedDatabase { connection, schema_version, recovered })
}

/// Current wall-clock time in milliseconds since the Unix epoch.
pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

fn open_connection(path: &Path) -> Result<Connection, SqliteError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|source| SqliteError::Io { path: parent.to_path_buf(), source })?;
        }
    }
    let connection = Connection::open(path).map_err(|source| classify_open_error(path, source))?;
    // Force a read so a non-database file is detected here, not later.
    match connection.query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get::<_, i64>(0))
    {
        Ok(_) => Ok(connection),
        Err(source) => Err(classify_open_error(path, source)),
    }
}

fn classify_open_error(path: &Path, source: rusqlite::Error) -> SqliteError {
    if is_not_a_database(&source) {
        SqliteError::NotADatabase { path: path.to_path_buf(), source }
    } else {
        SqliteError::Open { path: path.to_path_buf(), source }
    }
}

fn is_not_a_database(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(code, _) if code.code == rusqlite::ErrorCode::NotADatabase
    )
}

fn quarantine(path: &Path) -> Result<(), SqliteError> {
    let stamp = now_ms();
    let quarantine_path = path.with_file_name(format!(
        "{}.corrupt-{stamp}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("database")
    ));
    fs::rename(path, &quarantine_path)
        .map_err(|source| SqliteError::Io { path: quarantine_path, source })
}

fn run_migrations(
    path: &Path,
    connection: &mut Connection,
    migrations: &[Migration],
) -> Result<u32, SqliteError> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS migrations (\
             version INTEGER PRIMARY KEY, \
             applied_at_ms INTEGER NOT NULL);",
        )
        .map_err(|source| SqliteError::Migration { version: 0, source })?;

    let current: u32 = connection
        .query_row("SELECT COALESCE(MAX(version), 0) FROM migrations", [], |row| row.get(0))
        .map_err(|source| SqliteError::Migration { version: 0, source })?;

    let mut pending: Vec<&Migration> = migrations.iter().filter(|m| m.version > current).collect();
    pending.sort_by_key(|m| m.version);

    if pending.is_empty() {
        return Ok(current);
    }

    // Back up an existing, already-migrated database before the first pending
    // migration. A fresh database has nothing worth preserving.
    if current > 0 {
        backup_database(path, current)?;
    }

    let mut applied = current;
    for migration in pending {
        let transaction = connection
            .transaction()
            .map_err(|source| SqliteError::Migration { version: migration.version, source })?;
        if let Err(source) = transaction.execute_batch(migration.sql) {
            // Dropping the transaction rolls back every statement.
            return Err(SqliteError::Migration { version: migration.version, source });
        }
        transaction
            .execute(
                "INSERT INTO migrations (version, applied_at_ms) VALUES (?1, ?2)",
                params![migration.version, now_ms()],
            )
            .map_err(|source| SqliteError::Migration { version: migration.version, source })?;
        transaction
            .commit()
            .map_err(|source| SqliteError::Migration { version: migration.version, source })?;
        applied = migration.version;
    }
    Ok(applied)
}

fn backup_database(path: &Path, from_version: u32) -> Result<(), SqliteError> {
    let backup_path = path.with_file_name(format!(
        "{}.backup-{from_version}.sqlite",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("database")
    ));
    fs::copy(path, &backup_path)
        .map_err(|source| SqliteError::Backup { path: backup_path, source })?;
    Ok(())
}
