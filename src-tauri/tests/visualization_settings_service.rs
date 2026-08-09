use std::sync::{Arc, Mutex};

use pulseseek::visualization_settings_service::{
    InMemoryVisualizationSettingsService, NativeVisualizationSettingsService,
    VisualizationSettingsData, VisualizationSettingsService,
};
use pulseseek_cache::visualization_settings::{
    VisualizationSettingsCachePort, VisualizationSettingsError,
};
use pulseseek_domain::visualization::{
    VisualizationMode, VisualizationQuality, VisualizationSettings,
};

#[derive(Default)]
struct RecordingCache {
    value: Mutex<Option<VisualizationSettings>>,
}

impl VisualizationSettingsCachePort for RecordingCache {
    fn load_visualization_settings(
        &self,
    ) -> Result<Option<VisualizationSettings>, VisualizationSettingsError> {
        Ok(*self.value.lock().unwrap())
    }

    fn save_visualization_settings(
        &self,
        settings: VisualizationSettings,
    ) -> Result<(), VisualizationSettingsError> {
        *self.value.lock().unwrap() = Some(settings);
        Ok(())
    }
}

#[test]
fn missing_persisted_settings_load_safe_defaults() {
    let service = NativeVisualizationSettingsService::new(Arc::new(RecordingCache::default()));

    assert_eq!(service.load().unwrap(), VisualizationSettingsData::default());
}

#[test]
fn save_validates_and_round_trips_supported_values() {
    let service = NativeVisualizationSettingsService::new(Arc::new(RecordingCache::default()));
    let settings = VisualizationSettingsData {
        enabled: false,
        mode: "musical".to_string(),
        quality: "high".to_string(),
    };

    assert_eq!(service.save(settings.clone()).unwrap(), settings);
    assert_eq!(service.load().unwrap(), settings);
}

#[test]
fn save_rejects_unknown_values_without_overwriting_the_previous_record() {
    let cache = Arc::new(RecordingCache::default());
    let service = NativeVisualizationSettingsService::new(cache.clone());
    service.save(VisualizationSettingsData::default()).unwrap();

    assert!(service
        .save(VisualizationSettingsData {
            enabled: true,
            mode: "plugin".to_string(),
            quality: "extreme".to_string(),
        })
        .is_err());
    assert_eq!(
        *cache.value.lock().unwrap(),
        Some(VisualizationSettings::new(
            true,
            VisualizationMode::Waveform,
            VisualizationQuality::Balanced,
        ))
    );
}

#[test]
fn in_memory_fallback_persists_for_the_session() {
    let service = InMemoryVisualizationSettingsService::new();
    let settings = VisualizationSettingsData {
        enabled: true,
        mode: "linear".to_string(),
        quality: "low".to_string(),
    };

    service.save(settings.clone()).unwrap();
    assert_eq!(service.load().unwrap(), settings);
}
