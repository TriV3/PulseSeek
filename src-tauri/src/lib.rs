pub mod audio_device_service;
pub mod command_envelope;
pub mod diagnostics;
pub mod folder_enumeration_service;
pub mod playback_events;
pub mod playback_service;

use std::sync::Arc;

use diagnostics::DiagnosticsConfig;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _diagnostics_guard = diagnostics::init(DiagnosticsConfig::default());

    // Placeholder playback service — replaced with real implementation in a
    // later PR.
    let playback_service: std::sync::Mutex<Box<dyn playback_service::PlaybackService>> =
        std::sync::Mutex::new(Box::new(playback_service::FakePlaybackService::new()));

    // Placeholder audio device service — replaced with real implementation
    // in a later PR.
    let audio_device_service: std::sync::Mutex<Box<dyn audio_device_service::AudioDeviceService>> =
        std::sync::Mutex::new(Box::new(audio_device_service::FakeAudioDeviceService::new()));

    // Placeholder event emitter — replaced with a Tauri-backed emitter once
    // a real AppHandle is available.
    let event_emitter: Arc<dyn playback_events::PlaybackEventEmitter> =
        Arc::new(playback_events::NoopEventEmitter);

    // Native folder enumeration service.
    let enum_service: std::sync::Mutex<
        Box<dyn folder_enumeration_service::FolderEnumerationService>,
    > = std::sync::Mutex::new(Box::new(
        folder_enumeration_service::NativeFolderEnumerationService::new(),
    ));

    let active_enumerations: folder_enumeration_service::ActiveEnumerations =
        folder_enumeration_service::ActiveEnumerations::new();

    tauri::Builder::default()
        .manage(playback_service)
        .manage(audio_device_service)
        .manage(enum_service)
        .manage(active_enumerations)
        .manage(event_emitter)
        .invoke_handler(tauri::generate_handler![
            diagnostics::report_error,
            command_envelope::invoke_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
