pub mod types;

use std::sync::Arc;

use pulseseek_domain::error::{ApplicationError, ErrorContract};

use crate::audio_device_service::AudioDeviceService;
pub use crate::command_handlers::browsing::handle_pick_folder;
use crate::command_handlers::{browsing, device, parse_payload, playback};
use crate::folder_enumeration_service::{ActiveEnumerations, FolderEnumerationService};
use crate::playback_events::PlaybackEventEmitter;
use crate::playback_service::PlaybackService;
use crate::trash_service::TrashService;
pub(crate) use types::*;

pub fn dispatch(
    envelope: CommandEnvelope,
    service: &mut dyn PlaybackService,
    device_service: &mut dyn AudioDeviceService,
    enum_service: &mut dyn FolderEnumerationService,
    trash_service: &dyn TrashService,
    active: &ActiveEnumerations,
    events: &Arc<dyn PlaybackEventEmitter>,
) -> CommandResponse {
    if envelope.version != CURRENT_COMMAND_VERSION {
        return CommandResponse::err(BoundaryError {
            category: "Unsupported".to_string(),
            message: format!(
                "Unsupported command version {}. Expected {}.",
                envelope.version, CURRENT_COMMAND_VERSION
            ),
            diagnostic_code: "command.version".to_string(),
        });
    }

    match envelope.command.as_str() {
        "health" => {
            let _request: HealthRequest = match parse_payload("health", envelope.payload) {
                Ok(request) => request,
                Err(response) => return response,
            };
            CommandResponse::ok(serde_json::to_value(HealthResponse { ready: true }).unwrap())
        },
        "play" | "pause" | "resume" | "stop" | "seek" | "volume" | "set_playback_mode" => {
            playback::handle(&envelope.command, envelope.payload, service, events)
        },
        "list_devices" | "current_device" | "select_device" => {
            device::handle(&envelope.command, envelope.payload, service, device_service, events)
        },
        "list_browser_roots" | "start_enumeration" | "cancel_enumeration" | "move_to_trash" => {
            browsing::handle(
                &envelope.command,
                envelope.payload,
                enum_service,
                trash_service,
                active,
                events,
            )
        },
        _ => CommandResponse::err(BoundaryError {
            category: "Unsupported".to_string(),
            message: format!("Unknown command: {}", envelope.command),
            diagnostic_code: "command.unknown".to_string(),
        }),
    }
}

pub fn from_application_error(err: &ApplicationError) -> BoundaryError {
    let descriptor = err.user_descriptor();
    let context = err.diagnostic_context();
    BoundaryError {
        category: format!("{:?}", descriptor.category()),
        message: descriptor.message().to_string(),
        diagnostic_code: context.code().to_string(),
    }
}

/// Async Tauri command that opens the native folder picker dialog via the
/// callback-based `pick_folder` API. Using `blocking_pick_folder` from the
/// async runtime causes a deadlock on macOS (`dispatch_sync` to the main
/// thread while the runtime thread is blocked). The callback fires on the
/// main thread, so we wait via a channel without ever blocking the main
/// thread ourselves.
#[tauri::command]
pub async fn pick_folder_dialog(app: tauri::AppHandle) -> Result<PickFolderResponse, ()> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog().file().pick_folder(move |file| {
        let _ = tx.send(file);
    });
    let picked = rx.recv().map_err(|_| ())?;
    let path_str = match picked {
        Some(file_path) => {
            let s = file_path.to_string();
            match crate::path_validation::validate_directory(&s) {
                Ok(()) => Some(s),
                Err(_) => return Err(()),
            }
        },
        None => None,
    };
    Ok(PickFolderResponse { path: path_str })
}

/// Tauri command: dispatches a versioned command envelope and returns a
/// versioned response. This is the single entry point for all frontend
/// commands.
#[tauri::command]
pub fn invoke_command(
    envelope: CommandEnvelope,
    state: tauri::State<'_, std::sync::Mutex<Box<dyn PlaybackService>>>,
    device_state: tauri::State<'_, std::sync::Mutex<Box<dyn AudioDeviceService>>>,
    enum_state: tauri::State<'_, std::sync::Mutex<Box<dyn FolderEnumerationService>>>,
    trash_state: tauri::State<'_, std::sync::Mutex<Box<dyn TrashService>>>,
    active: tauri::State<'_, ActiveEnumerations>,
    events: tauri::State<'_, Arc<dyn PlaybackEventEmitter>>,
) -> CommandResponse {
    let mut service = state.lock().expect("playback service lock poisoned");
    let mut device_service = device_state.lock().expect("audio device service lock poisoned");
    let mut enum_service = enum_state.lock().expect("enumeration service lock poisoned");
    let trash_service = trash_state.lock().expect("trash service lock poisoned");
    dispatch(
        envelope,
        &mut **service,
        &mut **device_service,
        &mut **enum_service,
        &**trash_service,
        &active,
        &events,
    )
}

#[cfg(test)]
mod tests;
