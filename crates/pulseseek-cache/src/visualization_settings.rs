//! Persistence port and SQLite operations for visualization settings.

use std::fmt;

use pulseseek_domain::visualization::{
    VisualizationMode, VisualizationQuality, VisualizationSettings,
};
use rusqlite::params;

use crate::sqlite::OpenedDatabase;

#[derive(Debug)]
pub enum VisualizationSettingsError {
    InvalidMode(String),
    InvalidQuality(String),
    Sqlite(rusqlite::Error),
    WorkerStopped,
}

impl fmt::Display for VisualizationSettingsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMode(mode) => write!(formatter, "unknown visualization mode: {mode}"),
            Self::InvalidQuality(quality) => {
                write!(formatter, "unknown visualization quality: {quality}")
            },
            Self::Sqlite(error) => {
                write!(formatter, "visualization settings query failed: {error}")
            },
            Self::WorkerStopped => formatter.write_str("visualization settings worker stopped"),
        }
    }
}

impl std::error::Error for VisualizationSettingsError {}

pub trait VisualizationSettingsCachePort: Send + Sync {
    fn load_visualization_settings(
        &self,
    ) -> Result<Option<VisualizationSettings>, VisualizationSettingsError>;

    fn save_visualization_settings(
        &self,
        settings: VisualizationSettings,
    ) -> Result<(), VisualizationSettingsError>;
}

pub(crate) fn load_visualization_settings_db(
    database: &mut OpenedDatabase,
) -> Result<Option<VisualizationSettings>, VisualizationSettingsError> {
    let result = database.connection().query_row(
        "SELECT enabled, mode, quality FROM visualization_settings WHERE singleton = 1",
        [],
        |row| Ok((row.get::<_, bool>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)),
    );
    let (enabled, mode, quality) = match result {
        Ok(values) => values,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
        Err(error) => return Err(VisualizationSettingsError::Sqlite(error)),
    };
    let mode = VisualizationMode::from_id(&mode)
        .ok_or_else(|| VisualizationSettingsError::InvalidMode(mode))?;
    let quality = VisualizationQuality::from_id(&quality)
        .ok_or_else(|| VisualizationSettingsError::InvalidQuality(quality))?;
    Ok(Some(VisualizationSettings::new(enabled, mode, quality)))
}

pub(crate) fn save_visualization_settings_db(
    database: &mut OpenedDatabase,
    settings: VisualizationSettings,
) -> Result<(), VisualizationSettingsError> {
    database
        .connection()
        .execute(
            "INSERT INTO visualization_settings (singleton, enabled, mode, quality) \
             VALUES (1, ?1, ?2, ?3) \
             ON CONFLICT(singleton) DO UPDATE SET enabled = excluded.enabled, \
             mode = excluded.mode, quality = excluded.quality",
            params![settings.enabled, settings.mode.id(), settings.quality.id()],
        )
        .map_err(VisualizationSettingsError::Sqlite)?;
    Ok(())
}
