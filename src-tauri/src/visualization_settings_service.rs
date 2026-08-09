use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use crate::command_envelope::{from_application_error, BoundaryError, CURRENT_COMMAND_VERSION};
use crate::playback_service::PlaybackService;
use pulseseek_cache::visualization_settings::{
    VisualizationSettingsCachePort, VisualizationSettingsError,
};
use pulseseek_domain::error::{ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory};
use pulseseek_domain::visualization::{
    VisualizationMode, VisualizationQuality, VisualizationSettings,
};

pub type SharedVisualizationSettingsService = Arc<Mutex<Box<dyn VisualizationSettingsService>>>;

#[derive(Debug, serde::Serialize)]
pub struct VisualizationSettingsResponse {
    pub version: u32,
    pub settings: VisualizationSettingsData,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct VisualizationSettingsData {
    pub enabled: bool,
    pub mode: String,
    pub quality: String,
}

impl Default for VisualizationSettingsData {
    fn default() -> Self {
        Self::from(VisualizationSettings::default())
    }
}

impl From<VisualizationSettings> for VisualizationSettingsData {
    fn from(settings: VisualizationSettings) -> Self {
        Self {
            enabled: settings.enabled,
            mode: settings.mode.id().to_string(),
            quality: settings.quality.id().to_string(),
        }
    }
}

impl TryFrom<VisualizationSettingsData> for VisualizationSettings {
    type Error = ApplicationError;

    fn try_from(data: VisualizationSettingsData) -> Result<Self, Self::Error> {
        let mode = VisualizationMode::from_id(&data.mode).ok_or_else(|| {
            invalid_settings(VisualizationSettingsServiceError::UnknownMode(data.mode.clone()))
        })?;
        let quality = VisualizationQuality::from_id(&data.quality).ok_or_else(|| {
            invalid_settings(VisualizationSettingsServiceError::UnknownQuality(
                data.quality.clone(),
            ))
        })?;
        Ok(Self::new(data.enabled, mode, quality))
    }
}

pub trait VisualizationSettingsService: Send + Sync {
    fn load(&self) -> Result<VisualizationSettingsData, ApplicationError>;
    fn save(
        &self,
        settings: VisualizationSettingsData,
    ) -> Result<VisualizationSettingsData, ApplicationError>;
}

pub struct NativeVisualizationSettingsService {
    cache: Arc<dyn VisualizationSettingsCachePort>,
}

impl NativeVisualizationSettingsService {
    pub fn new(cache: Arc<dyn VisualizationSettingsCachePort>) -> Self {
        Self { cache }
    }
}

impl VisualizationSettingsService for NativeVisualizationSettingsService {
    fn load(&self) -> Result<VisualizationSettingsData, ApplicationError> {
        self.cache
            .load_visualization_settings()
            .map_err(cache_error)
            .map(|settings| settings.unwrap_or_default().into())
    }

    fn save(
        &self,
        settings: VisualizationSettingsData,
    ) -> Result<VisualizationSettingsData, ApplicationError> {
        let settings = VisualizationSettings::try_from(settings)?;
        self.cache.save_visualization_settings(settings).map_err(cache_error)?;
        Ok(settings.into())
    }
}

pub struct InMemoryVisualizationSettingsService {
    settings: Mutex<VisualizationSettings>,
}

impl InMemoryVisualizationSettingsService {
    pub fn new() -> Self {
        Self { settings: Mutex::new(VisualizationSettings::default()) }
    }
}

impl Default for InMemoryVisualizationSettingsService {
    fn default() -> Self {
        Self::new()
    }
}

impl VisualizationSettingsService for InMemoryVisualizationSettingsService {
    fn load(&self) -> Result<VisualizationSettingsData, ApplicationError> {
        Ok((*self.settings.lock().expect("visualization settings lock poisoned")).into())
    }

    fn save(
        &self,
        settings: VisualizationSettingsData,
    ) -> Result<VisualizationSettingsData, ApplicationError> {
        let settings = VisualizationSettings::try_from(settings)?;
        *self.settings.lock().expect("visualization settings lock poisoned") = settings;
        Ok(settings.into())
    }
}

fn cache_error(error: VisualizationSettingsError) -> ApplicationError {
    ApplicationError::new(
        ErrorCategory::Unavailable,
        DiagnosticContext::new(DiagnosticCode::VisualizationPreferences),
        error,
    )
}

fn invalid_settings(error: VisualizationSettingsServiceError) -> ApplicationError {
    ApplicationError::new(
        ErrorCategory::InvalidInput,
        DiagnosticContext::new(DiagnosticCode::VisualizationPreferences),
        error,
    )
}

#[derive(Debug)]
enum VisualizationSettingsServiceError {
    UnknownMode(String),
    UnknownQuality(String),
}

impl fmt::Display for VisualizationSettingsServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownMode(mode) => write!(formatter, "unknown visualization mode: {mode}"),
            Self::UnknownQuality(quality) => {
                write!(formatter, "unknown visualization quality: {quality}")
            },
        }
    }
}

impl Error for VisualizationSettingsServiceError {}

fn effective_runtime_settings(
    data: &VisualizationSettingsData,
    reduced_motion: bool,
) -> Result<VisualizationSettings, ApplicationError> {
    let mut settings = VisualizationSettings::try_from(data.clone())?;
    settings.enabled =
        settings.enabled && settings.mode != VisualizationMode::Waveform && !reduced_motion;
    Ok(settings)
}

#[tauri::command]
pub async fn load_visualization_settings(
    reduced_motion: bool,
    state: tauri::State<'_, SharedVisualizationSettingsService>,
    playback: tauri::State<'_, Arc<Mutex<Box<dyn PlaybackService>>>>,
) -> Result<VisualizationSettingsResponse, BoundaryError> {
    let state = Arc::clone(state.inner());
    let playback = Arc::clone(playback.inner());
    tauri::async_runtime::spawn_blocking(move || {
        let settings = state
            .lock()
            .map_err(|_| settings_lock_error())?
            .load()
            .map_err(|error| from_application_error(&error))?;
        let runtime = effective_runtime_settings(&settings, reduced_motion)
            .map_err(|error| from_application_error(&error))?;
        playback
            .lock()
            .map_err(|_| settings_lock_error())?
            .set_visualization_settings(runtime)
            .map_err(|error| from_application_error(&error))?;
        Ok(VisualizationSettingsResponse { version: CURRENT_COMMAND_VERSION, settings })
    })
    .await
    .map_err(|_| settings_lock_error())?
}

#[tauri::command]
pub async fn save_visualization_settings(
    settings: VisualizationSettingsData,
    reduced_motion: bool,
    state: tauri::State<'_, SharedVisualizationSettingsService>,
    playback: tauri::State<'_, Arc<Mutex<Box<dyn PlaybackService>>>>,
) -> Result<VisualizationSettingsResponse, BoundaryError> {
    let state = Arc::clone(state.inner());
    let playback = Arc::clone(playback.inner());
    tauri::async_runtime::spawn_blocking(move || {
        let settings = state
            .lock()
            .map_err(|_| settings_lock_error())?
            .save(settings)
            .map_err(|error| from_application_error(&error))?;
        let runtime = effective_runtime_settings(&settings, reduced_motion)
            .map_err(|error| from_application_error(&error))?;
        playback
            .lock()
            .map_err(|_| settings_lock_error())?
            .set_visualization_settings(runtime)
            .map_err(|error| from_application_error(&error))?;
        Ok(VisualizationSettingsResponse { version: CURRENT_COMMAND_VERSION, settings })
    })
    .await
    .map_err(|_| settings_lock_error())?
}

fn settings_lock_error() -> BoundaryError {
    BoundaryError {
        category: "Internal".to_string(),
        message: "PulseSeek could not apply visualization settings.".to_string(),
        diagnostic_code: "visualization.preferences".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn analyzer_settings() -> VisualizationSettingsData {
        VisualizationSettingsData {
            enabled: true,
            mode: "linear".to_string(),
            quality: "high".to_string(),
        }
    }

    #[test]
    fn analyzer_is_active_only_when_enabled_without_reduced_motion() {
        assert!(effective_runtime_settings(&analyzer_settings(), false).unwrap().enabled);

        let mut disabled = analyzer_settings();
        disabled.enabled = false;
        assert!(!effective_runtime_settings(&disabled, false).unwrap().enabled);
        assert!(!effective_runtime_settings(&analyzer_settings(), true).unwrap().enabled);
    }

    #[test]
    fn waveform_selection_keeps_fft_input_inactive() {
        let mut waveform = analyzer_settings();
        waveform.mode = "waveform".to_string();

        assert!(!effective_runtime_settings(&waveform, false).unwrap().enabled);
    }
}
