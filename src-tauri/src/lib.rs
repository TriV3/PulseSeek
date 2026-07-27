pub mod command_envelope;
pub mod diagnostics;
pub mod playback_service;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _diagnostics_guard = diagnostics::init(DiagnosticsConfig::default());

    // Placeholder playback service — replaced with real implementation in a
    // later PR.
    let playback_service: std::sync::Mutex<Box<dyn playback_service::PlaybackService>> =
        std::sync::Mutex::new(Box::new(playback_service::FakePlaybackService::new()));

    tauri::Builder::default()
        .manage(playback_service)
        .invoke_handler(tauri::generate_handler![
            diagnostics::report_error,
            command_envelope::invoke_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

use diagnostics::DiagnosticsConfig;
