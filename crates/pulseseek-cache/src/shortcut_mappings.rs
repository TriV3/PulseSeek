//! Persistence port and SQLite operations for one complete shortcut profile.

use std::fmt;

use pulseseek_domain::shortcuts::{
    default_shortcut_mappings, validate_complete_and_normalize_shortcut_mappings, Platform,
    ShortcutAction, ShortcutChord, ShortcutError, ShortcutMapping,
};
use rusqlite::params;

use crate::sqlite::OpenedDatabase;

#[derive(Debug)]
pub enum ShortcutMappingsError {
    Validation(ShortcutError),
    InvalidAction(String),
    Sqlite(rusqlite::Error),
    WorkerStopped,
}

impl fmt::Display for ShortcutMappingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(error) => write!(f, "shortcut mapping validation failed: {error}"),
            Self::InvalidAction(action) => {
                write!(f, "shortcut cache contains unknown action: {action}")
            },
            Self::Sqlite(error) => write!(f, "shortcut mapping cache query failed: {error}"),
            Self::WorkerStopped => write!(f, "shortcut mapping cache worker stopped"),
        }
    }
}

impl std::error::Error for ShortcutMappingsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Validation(error) => Some(error),
            Self::Sqlite(error) => Some(error),
            Self::InvalidAction(_) | Self::WorkerStopped => None,
        }
    }
}

/// Port for replacing, loading, and resetting one shortcut profile.
pub trait ShortcutMappingsCachePort: Send + Sync {
    /// Validates and atomically replaces every persisted mapping.
    fn replace_shortcut_mappings(
        &self,
        mappings: &[ShortcutMapping],
        platform: Platform,
    ) -> Result<(), ShortcutMappingsError>;

    /// Loads persisted mappings in replacement order.
    fn load_shortcut_mappings(&self) -> Result<Vec<ShortcutMapping>, ShortcutMappingsError>;

    /// Atomically replaces persisted mappings with canonical defaults.
    fn reset_shortcut_mappings(&self, platform: Platform) -> Result<(), ShortcutMappingsError>;
}

pub(crate) fn replace_shortcut_mappings_db(
    database: &mut OpenedDatabase,
    mappings: &[ShortcutMapping],
    platform: Platform,
) -> Result<(), ShortcutMappingsError> {
    let normalized = validate_complete_and_normalize_shortcut_mappings(mappings, platform)
        .map_err(ShortcutMappingsError::Validation)?;
    let transaction = database.connection().transaction().map_err(ShortcutMappingsError::Sqlite)?;
    transaction
        .execute("DELETE FROM shortcut_mappings", [])
        .map_err(ShortcutMappingsError::Sqlite)?;
    {
        let mut statement = transaction
            .prepare(
                "INSERT INTO shortcut_mappings \
                 (action, key, primary_modifier, shift_modifier, alt_modifier) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .map_err(ShortcutMappingsError::Sqlite)?;
        for mapping in normalized {
            statement
                .execute(params![
                    mapping.action.id(),
                    mapping.chord.key,
                    mapping.chord.primary,
                    mapping.chord.shift,
                    mapping.chord.alt,
                ])
                .map_err(ShortcutMappingsError::Sqlite)?;
        }
    }
    transaction.commit().map_err(ShortcutMappingsError::Sqlite)
}

pub(crate) fn load_shortcut_mappings_db(
    database: &mut OpenedDatabase,
) -> Result<Vec<ShortcutMapping>, ShortcutMappingsError> {
    let mut statement = database
        .connection()
        .prepare(
            "SELECT action, key, primary_modifier, shift_modifier, alt_modifier \
             FROM shortcut_mappings ORDER BY rowid",
        )
        .map_err(ShortcutMappingsError::Sqlite)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, bool>(2)?,
                row.get::<_, bool>(3)?,
                row.get::<_, bool>(4)?,
            ))
        })
        .map_err(ShortcutMappingsError::Sqlite)?;

    rows.map(|row| {
        let (action_id, key, primary, shift, alt) = row.map_err(ShortcutMappingsError::Sqlite)?;
        let action = ShortcutAction::from_id(&action_id)
            .ok_or_else(|| ShortcutMappingsError::InvalidAction(action_id))?;
        Ok(ShortcutMapping::new(action, ShortcutChord::new(key, primary, shift, alt)))
    })
    .collect()
}

pub(crate) fn reset_shortcut_mappings_db(
    database: &mut OpenedDatabase,
    platform: Platform,
) -> Result<(), ShortcutMappingsError> {
    replace_shortcut_mappings_db(database, &default_shortcut_mappings(), platform)
}
