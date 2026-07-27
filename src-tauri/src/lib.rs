pub mod command_envelope;
pub mod diagnostics;
pub mod playback_events;
pub mod playback_service;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _diagnostics_guard = diagnostics::init(DiagnosticsConfig::default());

    // Placeholder playback service — replaced with real implementation in a
    // later PR.
    let playback_service: std::sync::Mutex<Box<dyn playback_service::PlaybackService>> =
        std::sync::Mutex::new(Box::new(playback_service::FakePlaybackService::new()));

    // Placeholder event emitter — replaced with a Tauri-backed emitter once
    // a real AppHandle is available.
    let event_emitter: Box<dyn playback_events::PlaybackEventEmitter> =
        Box::new(playback_events::NoopEventEmitter);

    tauri::Builder::default()
        .manage(playback_service)
        .manage(event_emitter)
        .invoke_handler(tauri::generate_handler![
            diagnostics::report_error,
            command_envelope::invoke_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

use diagnostics::DiagnosticsConfig;
