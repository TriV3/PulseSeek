use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::command_envelope::{BoundaryError, CURRENT_COMMAND_VERSION};

const PREFERENCES_SCHEMA_VERSION: u32 = 1;

fn default_theme() -> String {
    "system".to_string()
}

fn default_waveform_style() -> String {
    "outline".to_string()
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PlayerPreferences {
    pub schema_version: u32,
    pub revision: u64,
    pub playback_mode: String,
    pub output_device_id: Option<String>,
    pub volume: f64,
    pub muted: bool,
    pub waveform_size: u8,
    pub browser_size: u8,
    pub selected_folder_path: Option<String>,
    pub expanded_folder_paths: Vec<String>,
    pub last_played_file_path: Option<String>,
    #[serde(default)]
    pub last_played_position_ms: u64,
    #[serde(default)]
    pub last_played_duration_ms: Option<u64>,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_waveform_style")]
    pub waveform_style: String,
}

impl Default for PlayerPreferences {
    fn default() -> Self {
        Self {
            schema_version: PREFERENCES_SCHEMA_VERSION,
            revision: 0,
            playback_mode: "one-shot".to_string(),
            output_device_id: None,
            volume: 1.0,
            muted: false,
            waveform_size: 38,
            browser_size: 24,
            selected_folder_path: None,
            expanded_folder_paths: Vec::new(),
            last_played_file_path: None,
            last_played_position_ms: 0,
            last_played_duration_ms: None,
            theme: default_theme(),
            waveform_style: default_waveform_style(),
        }
    }
}

impl PlayerPreferences {
    fn validated(mut self) -> Self {
        if !matches!(
            self.playback_mode.as_str(),
            "one-shot" | "loop-current" | "sequential" | "random"
        ) {
            self.playback_mode = "one-shot".to_string();
        }
        if !matches!(
            self.theme.as_str(),
            "system" | "light" | "dark" | "midnight" | "high-contrast"
        ) {
            self.theme = default_theme();
        }
        if !matches!(self.waveform_style.as_str(), "solid" | "gradient" | "outline") {
            self.waveform_style = default_waveform_style();
        }
        if !self.volume.is_finite() {
            self.volume = 1.0;
        }
        self.volume = self.volume.clamp(0.0, 1.0);
        self.waveform_size = self.waveform_size.clamp(22, 62);
        self.browser_size = self.browser_size.clamp(16, 46);
        self.expanded_folder_paths.sort();
        self.expanded_folder_paths.dedup();
        if self.last_played_file_path.is_none() {
            self.last_played_position_ms = 0;
            self.last_played_duration_ms = None;
        }
        self.schema_version = PREFERENCES_SCHEMA_VERSION;
        self
    }
}

pub trait PlayerPreferencesRepository: Send {
    fn load(&self) -> io::Result<PlayerPreferences>;
    fn save(&mut self, preferences: PlayerPreferences) -> io::Result<PlayerPreferences>;
}

pub type SharedPlayerPreferencesRepository = Arc<Mutex<Box<dyn PlayerPreferencesRepository>>>;

pub struct JsonPlayerPreferencesRepository {
    path: PathBuf,
}

impl JsonPlayerPreferencesRepository {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn write_atomically(path: &Path, preferences: &PlayerPreferences) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension("json.tmp");
        let mut file = File::create(&temporary)?;
        serde_json::to_writer_pretty(&mut file, preferences).map_err(io::Error::other)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        fs::rename(temporary, path)
    }
}

impl PlayerPreferencesRepository for JsonPlayerPreferencesRepository {
    fn load(&self) -> io::Result<PlayerPreferences> {
        match fs::read(&self.path) {
            Ok(bytes) => Ok(serde_json::from_slice::<PlayerPreferences>(&bytes)
                .unwrap_or_default()
                .validated()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Ok(PlayerPreferences::default())
            },
            Err(error) => Err(error),
        }
    }

    fn save(&mut self, preferences: PlayerPreferences) -> io::Result<PlayerPreferences> {
        let preferences = preferences.validated();
        let persisted = self.load()?;
        if preferences.revision < persisted.revision {
            return Ok(persisted);
        }
        Self::write_atomically(&self.path, &preferences)?;
        Ok(preferences)
    }
}

#[derive(Debug, Serialize)]
pub struct PlayerPreferencesResponse {
    pub version: u32,
    pub preferences: PlayerPreferences,
}

fn persistence_error() -> BoundaryError {
    BoundaryError {
        category: "Internal".to_string(),
        message: "PulseSeek could not save player preferences.".to_string(),
        diagnostic_code: "preferences.persistence".to_string(),
    }
}

#[tauri::command]
pub async fn load_player_preferences(
    state: tauri::State<'_, SharedPlayerPreferencesRepository>,
) -> Result<PlayerPreferencesResponse, BoundaryError> {
    let repository = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        let repository = repository.lock().map_err(|_| persistence_error())?;
        let preferences = repository.load().map_err(|_| persistence_error())?;
        Ok(PlayerPreferencesResponse { version: CURRENT_COMMAND_VERSION, preferences })
    })
    .await
    .map_err(|_| persistence_error())?
}

#[tauri::command]
pub async fn save_player_preferences(
    preferences: PlayerPreferences,
    state: tauri::State<'_, SharedPlayerPreferencesRepository>,
) -> Result<PlayerPreferencesResponse, BoundaryError> {
    let repository = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        let mut repository = repository.lock().map_err(|_| persistence_error())?;
        let preferences = repository.save(preferences).map_err(|_| persistence_error())?;
        Ok(PlayerPreferencesResponse { version: CURRENT_COMMAND_VERSION, preferences })
    })
    .await
    .map_err(|_| persistence_error())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_corrupt_file_uses_safe_defaults() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("player-preferences.json");
        let repository = JsonPlayerPreferencesRepository::new(path.clone());
        assert_eq!(repository.load().unwrap(), PlayerPreferences::default());

        fs::write(path, b"not json").unwrap();
        assert_eq!(repository.load().unwrap(), PlayerPreferences::default());
    }

    #[test]
    fn save_is_immediate_atomic_and_rejects_stale_revision() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("player-preferences.json");
        let mut repository = JsonPlayerPreferencesRepository::new(path.clone());
        let newest =
            PlayerPreferences { revision: 2, volume: 0.35, ..PlayerPreferences::default() };
        repository.save(newest.clone()).unwrap();

        assert_eq!(repository.load().unwrap().volume, 0.35);
        assert!(!path.with_extension("json.tmp").exists());

        let mut restarted_repository = JsonPlayerPreferencesRepository::new(path.clone());
        let mut stale = newest;
        stale.revision = 1;
        stale.volume = 0.9;
        assert_eq!(restarted_repository.save(stale).unwrap().volume, 0.35);
        assert_eq!(restarted_repository.load().unwrap().volume, 0.35);
    }

    #[test]
    fn theme_defaults_to_system() {
        assert_eq!(PlayerPreferences::default().theme, "system");
    }

    #[test]
    fn waveform_style_defaults_to_outline() {
        assert_eq!(PlayerPreferences::default().waveform_style, "outline");
    }

    #[test]
    fn validated_waveform_style_accepts_supported_values() {
        for style in ["solid", "gradient", "outline"] {
            let preferences = PlayerPreferences {
                waveform_style: style.to_string(),
                ..PlayerPreferences::default()
            }
            .validated();
            assert_eq!(preferences.waveform_style, style);
        }
    }

    #[test]
    fn validated_waveform_style_falls_back_to_outline_for_unknown_values() {
        let preferences = PlayerPreferences {
            waveform_style: "neon".to_string(),
            ..PlayerPreferences::default()
        }
        .validated();
        assert_eq!(preferences.waveform_style, "outline");
    }

    #[test]
    fn waveform_style_round_trips_through_repository() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("player-preferences.json");
        let mut repository = JsonPlayerPreferencesRepository::new(path.clone());
        let preferences = PlayerPreferences {
            waveform_style: "gradient".to_string(),
            ..PlayerPreferences::default()
        };
        repository.save(preferences.clone()).unwrap();
        assert_eq!(repository.load().unwrap().waveform_style, "gradient");
    }

    #[test]
    fn validated_theme_accepts_supported_values() {
        for theme in ["system", "light", "dark", "midnight", "high-contrast"] {
            let preferences =
                PlayerPreferences { theme: theme.to_string(), ..PlayerPreferences::default() }
                    .validated();
            assert_eq!(preferences.theme, theme);
        }
    }

    #[test]
    fn validated_theme_falls_back_to_system_for_unknown_values() {
        let preferences = PlayerPreferences {
            theme: "midnight-blue".to_string(),
            ..PlayerPreferences::default()
        }
        .validated();
        assert_eq!(preferences.theme, "system");
    }

    #[test]
    fn midnight_theme_round_trips_through_repository() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("player-preferences.json");
        let mut repository = JsonPlayerPreferencesRepository::new(path.clone());
        let preferences =
            PlayerPreferences { theme: "midnight".to_string(), ..PlayerPreferences::default() };
        repository.save(preferences.clone()).unwrap();
        assert_eq!(repository.load().unwrap().theme, "midnight");
    }

    #[test]
    fn high_contrast_theme_round_trips_through_repository() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("player-preferences.json");
        let mut repository = JsonPlayerPreferencesRepository::new(path.clone());
        let preferences = PlayerPreferences {
            theme: "high-contrast".to_string(),
            ..PlayerPreferences::default()
        };
        repository.save(preferences.clone()).unwrap();
        assert_eq!(repository.load().unwrap().theme, "high-contrast");
    }

    #[test]
    fn legacy_file_without_theme_field_deserializes_with_default() {
        let serialized = r#"{
            "schema_version": 1,
            "revision": 3,
            "playback_mode": "loop-current",
            "output_device_id": null,
            "volume": 0.5,
            "muted": true,
            "waveform_size": 40,
            "browser_size": 25,
            "selected_folder_path": "/music",
            "expanded_folder_paths": ["/music"],
            "last_played_file_path": "/music/track.wav"
        }"#;
        let preferences: PlayerPreferences = serde_json::from_str(serialized).unwrap();
        assert_eq!(preferences.theme, "system");
        assert_eq!(preferences.waveform_style, "outline");
        assert_eq!(preferences.revision, 3);
        assert_eq!(preferences.volume, 0.5);
    }

    #[test]
    fn theme_round_trips_through_repository() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("player-preferences.json");
        let mut repository = JsonPlayerPreferencesRepository::new(path.clone());
        let preferences =
            PlayerPreferences { theme: "dark".to_string(), ..PlayerPreferences::default() };
        repository.save(preferences.clone()).unwrap();
        assert_eq!(repository.load().unwrap().theme, "dark");
    }

    #[test]
    fn validation_persists_position_but_excludes_transport_state() {
        let serialized = serde_json::to_value(PlayerPreferences::default()).unwrap();
        assert_eq!(
            serialized.get("last_played_position_ms").and_then(|value| value.as_u64()),
            Some(0)
        );
        assert!(serialized.get("transport_state").is_none());
        assert!(serialized.get("playing").is_none());
    }
}
