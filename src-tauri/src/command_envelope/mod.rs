pub mod types;

use std::sync::Arc;

use pulseseek_domain::error::{ApplicationError, ErrorContract};
use pulseseek_domain::playback::mode::PlaybackMode;

use crate::audio_device_service::AudioDeviceService;
use crate::dialog_service::FolderPicker;
use crate::folder_enumeration_service::{ActiveEnumerations, FolderEnumerationService};
use crate::playback_events::{DeviceLostPayload, PlaybackEventEmitter, EVENT_DEVICE_LOST};
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
            let _request: HealthRequest = match serde_json::from_value(envelope.payload) {
                Ok(r) => r,
                Err(e) => {
                    return CommandResponse::err(BoundaryError {
                        category: "InvalidInput".to_string(),
                        message: format!("Invalid health command payload: {}", e),
                        diagnostic_code: "command.payload".to_string(),
                    });
                },
            };
            CommandResponse::ok(serde_json::to_value(HealthResponse { ready: true }).unwrap())
        },
        "play" => {
            let request: PlayRequest = match serde_json::from_value(envelope.payload) {
                Ok(r) => r,
                Err(e) => {
                    return CommandResponse::err(BoundaryError {
                        category: "InvalidInput".to_string(),
                        message: format!("Invalid play command payload: {}", e),
                        diagnostic_code: "command.payload".to_string(),
                    });
                },
            };
            match service.play(&request.path) {
                Ok(()) => {
                    let _ = events.emit_state("playing");
                    CommandResponse::ok(serde_json::to_value(PlayResponse {}).unwrap())
                },
                Err(e) => CommandResponse::err(from_application_error(&e)),
            }
        },
        "pause" => {
            let _request: PauseRequest = match serde_json::from_value(envelope.payload) {
                Ok(r) => r,
                Err(e) => {
                    return CommandResponse::err(BoundaryError {
                        category: "InvalidInput".to_string(),
                        message: format!("Invalid pause command payload: {}", e),
                        diagnostic_code: "command.payload".to_string(),
                    });
                },
            };
            match service.pause() {
                Ok(()) => {
                    let _ = events.emit_state("paused");
                    CommandResponse::ok(serde_json::to_value(PauseResponse {}).unwrap())
                },
                Err(e) => CommandResponse::err(from_application_error(&e)),
            }
        },
        "resume" => {
            let _request: ResumeRequest = match serde_json::from_value(envelope.payload) {
                Ok(r) => r,
                Err(e) => {
                    return CommandResponse::err(BoundaryError {
                        category: "InvalidInput".to_string(),
                        message: format!("Invalid resume command payload: {}", e),
                        diagnostic_code: "command.payload".to_string(),
                    });
                },
            };
            match service.resume() {
                Ok(()) => {
                    let _ = events.emit_state("playing");
                    CommandResponse::ok(serde_json::to_value(ResumeResponse {}).unwrap())
                },
                Err(e) => CommandResponse::err(from_application_error(&e)),
            }
        },
        "stop" => {
            let _request: StopRequest = match serde_json::from_value(envelope.payload) {
                Ok(r) => r,
                Err(e) => {
                    return CommandResponse::err(BoundaryError {
                        category: "InvalidInput".to_string(),
                        message: format!("Invalid stop command payload: {}", e),
                        diagnostic_code: "command.payload".to_string(),
                    });
                },
            };
            match service.stop() {
                Ok(()) => {
                    let _ = events.emit_state("stopped");
                    CommandResponse::ok(serde_json::to_value(StopResponse {}).unwrap())
                },
                Err(e) => CommandResponse::err(from_application_error(&e)),
            }
        },
        "seek" => {
            let request: SeekRequest = match serde_json::from_value(envelope.payload) {
                Ok(r) => r,
                Err(e) => {
                    return CommandResponse::err(BoundaryError {
                        category: "InvalidInput".to_string(),
                        message: format!("Invalid seek command payload: {}", e),
                        diagnostic_code: "command.payload".to_string(),
                    });
                },
            };
            match service.seek(request.position_ms) {
                Ok(position_ms) => {
                    CommandResponse::ok(serde_json::to_value(SeekResponse { position_ms }).unwrap())
                },
                Err(e) => CommandResponse::err(from_application_error(&e)),
            }
        },
        "volume" => {
            let request: VolumeRequest = match serde_json::from_value(envelope.payload) {
                Ok(r) => r,
                Err(e) => {
                    return CommandResponse::err(BoundaryError {
                        category: "InvalidInput".to_string(),
                        message: format!("Invalid volume command payload: {}", e),
                        diagnostic_code: "command.payload".to_string(),
                    });
                },
            };
            match service.set_volume(request.gain, request.muted) {
                Ok(()) => CommandResponse::ok(serde_json::to_value(VolumeResponse {}).unwrap()),
                Err(e) => CommandResponse::err(from_application_error(&e)),
            }
        },
        "set_playback_mode" => {
            let request: SetPlaybackModeRequest = match serde_json::from_value(envelope.payload) {
                Ok(r) => r,
                Err(e) => {
                    return CommandResponse::err(BoundaryError {
                        category: "InvalidInput".to_string(),
                        message: format!("Invalid set_playback_mode command payload: {}", e),
                        diagnostic_code: "command.payload".to_string(),
                    });
                },
            };
            let mode = match parse_playback_mode(&request.mode) {
                Some(mode) => mode,
                None => {
                    return CommandResponse::err(BoundaryError {
                        category: "InvalidInput".to_string(),
                        message: format!("Unknown playback mode: {}", request.mode),
                        diagnostic_code: "command.mode".to_string(),
                    });
                },
            };
            match service.set_mode(mode) {
                Ok(confirmed_mode) => CommandResponse::ok(
                    serde_json::to_value(SetPlaybackModeResponse {
                        mode: playback_mode_name(confirmed_mode),
                    })
                    .unwrap(),
                ),
                Err(e) => CommandResponse::err(from_application_error(&e)),
            }
        },
        "list_devices" => {
            let _request: ListDevicesRequest = match serde_json::from_value(envelope.payload) {
                Ok(r) => r,
                Err(e) => {
                    return CommandResponse::err(BoundaryError {
                        category: "InvalidInput".to_string(),
                        message: format!("Invalid list_devices command payload: {}", e),
                        diagnostic_code: "command.payload".to_string(),
                    });
                },
            };
            match device_service.list_devices() {
                Ok(devices) => CommandResponse::ok(
                    serde_json::to_value(ListDevicesResponse { devices }).unwrap(),
                ),
                Err(e) => CommandResponse::err(from_application_error(&e)),
            }
        },
        "current_device" => {
            let _request: CurrentDeviceRequest = match serde_json::from_value(envelope.payload) {
                Ok(r) => r,
                Err(e) => {
                    return CommandResponse::err(BoundaryError {
                        category: "InvalidInput".to_string(),
                        message: format!("Invalid current_device command payload: {}", e),
                        diagnostic_code: "command.payload".to_string(),
                    });
                },
            };
            match device_service.current_device() {
                Ok(device) => CommandResponse::ok(
                    serde_json::to_value(CurrentDeviceResponse { device }).unwrap(),
                ),
                Err(e) => CommandResponse::err(from_application_error(&e)),
            }
        },
        "select_device" => {
            let request: SelectDeviceRequest = match serde_json::from_value(envelope.payload) {
                Ok(r) => r,
                Err(e) => {
                    return CommandResponse::err(BoundaryError {
                        category: "InvalidInput".to_string(),
                        message: format!("Invalid select_device command payload: {}", e),
                        diagnostic_code: "command.payload".to_string(),
                    });
                },
            };
            match device_service.select_device(&request.device_id) {
                Ok(()) => {
                    if device_service.is_device_lost() {
                        let _ = events.emit(
                            EVENT_DEVICE_LOST,
                            serde_json::to_value(DeviceLostPayload {
                                previous_device_id: request.device_id.clone(),
                            })
                            .unwrap(),
                        );
                    }
                    CommandResponse::ok(serde_json::to_value(SelectDeviceResponse {}).unwrap())
                },
                Err(e) => CommandResponse::err(from_application_error(&e)),
            }
        },
        "start_enumeration" => {
            let request: StartEnumerationRequest = match serde_json::from_value(envelope.payload) {
                Ok(r) => r,
                Err(e) => {
                    return CommandResponse::err(BoundaryError {
                        category: "InvalidInput".to_string(),
                        message: format!("Invalid start_enumeration command payload: {}", e),
                        diagnostic_code: "command.payload".to_string(),
                    });
                },
            };
            let batch_size = request.batch_size.unwrap_or(100) as usize;
            match enum_service.start_enumeration(
                &request.path,
                batch_size,
                request.show_unsupported,
                active,
                events.clone(),
            ) {
                Ok(session_id) => CommandResponse::ok(
                    serde_json::to_value(StartEnumerationResponse { session_id }).unwrap(),
                ),
                Err(e) => CommandResponse::err(from_application_error(&e)),
            }
        },
        "cancel_enumeration" => {
            let request: CancelEnumerationRequest = match serde_json::from_value(envelope.payload) {
                Ok(r) => r,
                Err(e) => {
                    return CommandResponse::err(BoundaryError {
                        category: "InvalidInput".to_string(),
                        message: format!("Invalid cancel_enumeration command payload: {}", e),
                        diagnostic_code: "command.payload".to_string(),
                    });
                },
            };
            active.cancel(&request.session_id);
            CommandResponse::ok(serde_json::to_value(CancelEnumerationResponse {}).unwrap())
        },
        "move_to_trash" => {
            let request: MoveToTrashRequest = match serde_json::from_value(envelope.payload) {
                Ok(r) => r,
                Err(e) => {
                    return CommandResponse::err(BoundaryError {
                        category: "InvalidInput".to_string(),
                        message: format!("Invalid move_to_trash command payload: {}", e),
                        diagnostic_code: "command.payload".to_string(),
                    });
                },
            };
            if request.paths.is_empty() {
                return CommandResponse::ok(
                    serde_json::to_value(MoveToTrashResponse { results: vec![] }).unwrap(),
                );
            }
            let path_bufs: Vec<std::path::PathBuf> =
                request.paths.iter().map(std::path::PathBuf::from).collect();
            let results = trash_service.move_to_trash(path_bufs);
            let response_results: Vec<MoveToTrashItemResult> = results
                .into_iter()
                .map(|(path, result)| match result {
                    Ok(()) => MoveToTrashItemResult {
                        path: path.to_string_lossy().to_string(),
                        ok: true,
                        category: None,
                        message: None,
                        diagnostic_code: None,
                    },
                    Err(e) => {
                        let desc = e.user_descriptor();
                        let ctx = e.diagnostic_context();
                        MoveToTrashItemResult {
                            path: path.to_string_lossy().to_string(),
                            ok: false,
                            category: Some(format!("{:?}", desc.category())),
                            message: Some(desc.message().to_string()),
                            diagnostic_code: Some(ctx.code().to_string()),
                        }
                    },
                })
                .collect();
            CommandResponse::ok(
                serde_json::to_value(MoveToTrashResponse { results: response_results }).unwrap(),
            )
        },
        _ => CommandResponse::err(BoundaryError {
            category: "Unsupported".to_string(),
            message: format!("Unknown command: {}", envelope.command),
            diagnostic_code: "command.unknown".to_string(),
        }),
    }
}

fn parse_playback_mode(value: &str) -> Option<PlaybackMode> {
    match value {
        "one-shot" => Some(PlaybackMode::OneShot),
        "loop-current" => Some(PlaybackMode::LoopCurrent),
        "sequential" => Some(PlaybackMode::Sequential),
        "random" => Some(PlaybackMode::Random),
        _ => None,
    }
}

fn playback_mode_name(mode: PlaybackMode) -> String {
    match mode {
        PlaybackMode::OneShot => "one-shot",
        PlaybackMode::LoopCurrent => "loop-current",
        PlaybackMode::Sequential => "sequential",
        PlaybackMode::Random => "random",
    }
    .to_string()
}

/// Handles the `"pick_folder"` command: opens a native folder picker dialog
/// and returns the selected path (or `None` on cancellation).
///
/// Extracted as a standalone function so it can be unit-tested without
/// a running Tauri application.
pub fn handle_pick_folder(picker: &dyn FolderPicker) -> CommandResponse {
    match picker.pick_folder() {
        Ok(path) => CommandResponse::ok(serde_json::to_value(PickFolderResponse { path }).unwrap()),
        Err(e) => CommandResponse::err(from_application_error(&e)),
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
