pub mod audio_device_service;
pub mod command_envelope;
pub mod diagnostics;
pub mod dialog_service;
pub mod folder_enumeration_service;
pub mod native_audio_device_service;
pub mod path_validation;
pub mod playback_events;
pub mod playback_service;

use std::sync::Arc;

use diagnostics::DiagnosticsConfig;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _diagnostics_guard = diagnostics::init(DiagnosticsConfig::default());

    // Placeholder playback service — replaced with real implementation in a
    // later PR.
    let playback_service: std::sync::Mutex<Box<dyn playback_service::PlaybackService>> =
        std::sync::Mutex::new(Box::new(playback_service::FakePlaybackService::new()));

    // Native audio-device service. It falls back to the operating-system
    // default when a requested device is unavailable.
    let audio_device_service: std::sync::Mutex<Box<dyn audio_device_service::AudioDeviceService>> =
        std::sync::Mutex::new(
            Box::new(native_audio_device_service::NativeAudioDeviceService::new()),
        );

    // Native folder enumeration service.
    let enum_service: std::sync::Mutex<
        Box<dyn folder_enumeration_service::FolderEnumerationService>,
    > = std::sync::Mutex::new(Box::new(
        folder_enumeration_service::NativeFolderEnumerationService::new(),
    ));

    let active_enumerations: folder_enumeration_service::ActiveEnumerations =
        folder_enumeration_service::ActiveEnumerations::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(playback_service)
        .manage(audio_device_service)
        .manage(enum_service)
        .manage(active_enumerations)
        .invoke_handler(tauri::generate_handler![
            diagnostics::report_error,
            command_envelope::invoke_command
        ])
        .setup(|app| {
            // Real event emitter using Tauri's AppHandle. Replaces the
            // NoopEventEmitter used during early development.
            let event_emitter: Arc<dyn playback_events::PlaybackEventEmitter> =
                Arc::new(playback_events::TauriEventEmitter::new(app.handle().clone()));
            app.manage(event_emitter);

            // Native folder picker dialog backed by the OS.
            let folder_picker: std::sync::Mutex<Box<dyn dialog_service::FolderPicker>> =
                std::sync::Mutex::new(Box::new(dialog_service::TauriFolderPicker::new(
                    app.handle().clone(),
                )));
            app.manage(folder_picker);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
