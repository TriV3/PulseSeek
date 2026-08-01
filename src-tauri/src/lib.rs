pub mod audio_device_service;
pub mod command_envelope;
mod command_handlers;
pub mod diagnostics;
pub mod dialog_service;
pub mod folder_enumeration_service;
pub mod native_audio_device_service;
pub mod native_playback_service;
pub mod path_validation;
pub mod playback_events;
pub mod playback_service;
pub mod player_preferences;
pub mod trash_service;

use std::sync::{Arc, Mutex};

use diagnostics::DiagnosticsConfig;
use pulseseek_audio_cpal::CpalAudioOutput;
use tauri::Manager;

use crate::player_preferences::{
    JsonPlayerPreferencesRepository, SharedPlayerPreferencesRepository,
};
use crate::trash_service::{NativeTrashService, TrashService};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _diagnostics_guard = diagnostics::init(DiagnosticsConfig::default());

    // Shared native audio output. Both the device service and playback service
    // operate on the same underlying cpal output instance.
    let audio_output: Arc<Mutex<CpalAudioOutput>> = Arc::new(Mutex::new(CpalAudioOutput::new()));

    // Native playback service that owns decoder workers and drives the
    // shared cpal output. Events are wired during setup.
    let playback_service: std::sync::Mutex<Box<dyn playback_service::PlaybackService>> =
        std::sync::Mutex::new(Box::new(native_playback_service::NativePlaybackService::new(
            Arc::clone(&audio_output),
        )));

    // Native audio-device service. It falls back to the operating-system
    // default when a requested device is unavailable.
    let audio_device_service: std::sync::Mutex<Box<dyn audio_device_service::AudioDeviceService>> =
        std::sync::Mutex::new(Box::new(native_audio_device_service::NativeAudioDeviceService::new(
            audio_output,
        )));

    // Native folder enumeration service.
    let enum_service: std::sync::Mutex<
        Box<dyn folder_enumeration_service::FolderEnumerationService>,
    > = std::sync::Mutex::new(Box::new(
        folder_enumeration_service::NativeFolderEnumerationService::new(),
    ));

    let active_enumerations: folder_enumeration_service::ActiveEnumerations =
        folder_enumeration_service::ActiveEnumerations::new();

    // Native trash service backed by the operating system trash.
    let trash_service: std::sync::Mutex<Box<dyn TrashService>> = std::sync::Mutex::new(Box::new(
        NativeTrashService::new(pulseseek_browser_fs::trash::NativeFileTrash),
    ));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(playback_service)
        .manage(audio_device_service)
        .manage(enum_service)
        .manage(active_enumerations)
        .manage(trash_service)
        .invoke_handler(tauri::generate_handler![
            diagnostics::report_error,
            command_envelope::invoke_command,
            command_envelope::pick_folder_dialog,
            player_preferences::load_player_preferences,
            player_preferences::save_player_preferences
        ])
        .setup(|app| {
            let preferences_path = app
                .path()
                .app_config_dir()
                .expect("application config directory unavailable")
                .join("player-preferences.json");
            let preferences: SharedPlayerPreferencesRepository = Arc::new(Mutex::new(Box::new(
                JsonPlayerPreferencesRepository::new(preferences_path),
            )));
            app.manage(preferences);

            // Technical cache on its own worker. A cache failure must never
            // prevent Audio Player startup, so startup logs and continues.
            if let Ok(config_dir) = app.path().app_config_dir() {
                let cache_path = config_dir.join("app-cache.sqlite");
                match pulseseek_cache::technical_cache::TechnicalCache::start(&cache_path) {
                    Ok(cache) => {
                        let port: Arc<dyn pulseseek_cache::technical_cache::TechnicalCachePort> =
                            Arc::new(cache);
                        tracing::info!(status = ?port.status(), "technical cache ready");
                        app.manage(port);
                    },
                    Err(error) => {
                        tracing::warn!(
                            error = %error,
                            "technical cache unavailable; continuing without cache"
                        );
                    },
                }
            }

            // Real event emitter using Tauri's AppHandle.
            let event_emitter: Arc<dyn playback_events::PlaybackEventEmitter> =
                Arc::new(playback_events::TauriEventEmitter::new(app.handle().clone()));
            // Wire real events into the playback service before manage moves
            // the emitter into Tauri managed state.
            if let Ok(mut playback) =
                app.state::<std::sync::Mutex<Box<dyn playback_service::PlaybackService>>>().lock()
            {
                playback.set_events(Some(Arc::clone(&event_emitter)));
            }
            app.manage(event_emitter);

            // Native folder picker dialog backed by the OS. Wrapped in Arc so
            // the command handler can clone it and spawn a blocking dialog
            // without holding a MutexGuard across an async boundary.
            let folder_picker: std::sync::Arc<
                std::sync::Mutex<Box<dyn dialog_service::FolderPicker>>,
            > = std::sync::Arc::new(std::sync::Mutex::new(Box::new(
                dialog_service::TauriFolderPicker::new(app.handle().clone()),
            )));
            app.manage(folder_picker);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
