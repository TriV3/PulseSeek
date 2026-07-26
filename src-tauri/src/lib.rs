pub mod diagnostics;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _diagnostics_guard = diagnostics::init(DiagnosticsConfig::default());

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![diagnostics::report_error])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

use diagnostics::DiagnosticsConfig;
