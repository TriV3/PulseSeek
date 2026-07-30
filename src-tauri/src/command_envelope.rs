use std::sync::Arc;

use pulseseek_domain::error::{ApplicationError, ErrorContract};
use pulseseek_domain::playback::mode::PlaybackMode;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::audio_device_service::{AudioDeviceService, DeviceInfoData};
use crate::dialog_service::FolderPicker;
use crate::folder_enumeration_service::{ActiveEnumerations, FolderEnumerationService};
use crate::playback_events::{DeviceLostPayload, PlaybackEventEmitter, EVENT_DEVICE_LOST};
use crate::playback_service::PlaybackService;

pub const CURRENT_COMMAND_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
pub struct CommandEnvelope {
    pub version: u32,
    pub command: String,
    pub payload: Value,
}

#[derive(Debug, Serialize)]
pub struct CommandResponse {
    pub version: u32,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<BoundaryError>,
}

#[derive(Debug, Serialize)]
pub struct BoundaryError {
    pub category: String,
    pub message: String,
    pub diagnostic_code: String,
}

#[derive(Debug, Deserialize)]
pub struct HealthRequest {}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub ready: bool,
}

// ── Playback command request/response types ─────────────────────────────

#[derive(Debug, Deserialize)]
pub struct PlayRequest {
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct PlayResponse {}

#[derive(Debug, Deserialize)]
pub struct PauseRequest {}

#[derive(Debug, Serialize)]
pub struct PauseResponse {}

#[derive(Debug, Deserialize)]
pub struct ResumeRequest {}

#[derive(Debug, Serialize)]
pub struct ResumeResponse {}

#[derive(Debug, Deserialize)]
pub struct StopRequest {}

#[derive(Debug, Serialize)]
pub struct StopResponse {}

#[derive(Debug, Deserialize)]
pub struct SeekRequest {
    pub position_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct SeekResponse {
    pub position_ms: u64,
}

#[derive(Debug, Deserialize)]
pub struct VolumeRequest {
    pub gain: f64,
    pub muted: bool,
}

#[derive(Debug, Serialize)]
pub struct VolumeResponse {}

#[derive(Debug, Deserialize)]
pub struct SetPlaybackModeRequest {
    pub mode: String,
}

#[derive(Debug, Serialize)]
pub struct SetPlaybackModeResponse {
    pub mode: String,
}

// ── Audio device command request/response types ────────────────────────

#[derive(Debug, Deserialize)]
pub struct ListDevicesRequest {}

#[derive(Debug, Serialize)]
pub struct ListDevicesResponse {
    pub devices: Vec<DeviceInfoData>,
}

#[derive(Debug, Deserialize)]
pub struct CurrentDeviceRequest {}

#[derive(Debug, Serialize)]
pub struct CurrentDeviceResponse {
    pub device: Option<DeviceInfoData>,
}

#[derive(Debug, Deserialize)]
pub struct SelectDeviceRequest {
    pub device_id: String,
}

#[derive(Debug, Serialize)]
pub struct SelectDeviceResponse {}

// ── Folder picker command request/response types ──────────────────────

#[derive(Debug, Deserialize)]
pub struct PickFolderRequest {}

#[derive(Debug, Deserialize, Serialize)]
pub struct PickFolderResponse {
    pub path: Option<String>,
}

// ── Folder enumeration command request/response types ─────────────────

#[derive(Debug, Deserialize)]
pub struct StartEnumerationRequest {
    pub path: String,
    pub batch_size: Option<u64>,
    #[serde(default)]
    pub show_unsupported: bool,
}

#[derive(Debug, Serialize)]
pub struct StartEnumerationResponse {
    pub session_id: String,
}

#[derive(Debug, Deserialize)]
pub struct CancelEnumerationRequest {
    pub session_id: String,
}

#[derive(Debug, Serialize)]
pub struct CancelEnumerationResponse {}

fn parse_playback_mode(value: &str) -> Option<PlaybackMode> {
    match value {
        "one-shot" => Some(PlaybackMode::OneShot),
        "loop-current" => Some(PlaybackMode::LoopCurrent),
        "sequential" => Some(PlaybackMode::Sequential),
        "random" => Some(PlaybackMode::Random),
        _ => None,
    }
}

impl CommandResponse {
    pub fn ok(data: Value) -> Self {
        Self { version: CURRENT_COMMAND_VERSION, ok: true, data: Some(data), error: None }
    }

    pub fn err(error: BoundaryError) -> Self {
        Self { version: CURRENT_COMMAND_VERSION, ok: false, data: None, error: Some(error) }
    }
}

pub fn dispatch(
    envelope: CommandEnvelope,
    service: &mut dyn PlaybackService,
    device_service: &mut dyn AudioDeviceService,
    enum_service: &mut dyn FolderEnumerationService,
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
        _ => CommandResponse::err(BoundaryError {
            category: "Unsupported".to_string(),
            message: format!("Unknown command: {}", envelope.command),
            diagnostic_code: "command.unknown".to_string(),
        }),
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
    active: tauri::State<'_, ActiveEnumerations>,
    events: tauri::State<'_, Arc<dyn PlaybackEventEmitter>>,
) -> CommandResponse {
    let mut service = state.lock().expect("playback service lock poisoned");
    let mut device_service = device_state.lock().expect("audio device service lock poisoned");
    let mut enum_service = enum_state.lock().expect("enumeration service lock poisoned");
    dispatch(envelope, &mut **service, &mut **device_service, &mut **enum_service, &active, &events)
}

#[cfg(test)]
mod tests {
    use pulseseek_domain::error::{DiagnosticCode, DiagnosticContext, ErrorCategory};

    use super::*;
    use crate::audio_device_service::FakeAudioDeviceService;
    use crate::dialog_service::FakeFolderPicker;
    use crate::folder_enumeration_service::{ActiveEnumerations, FakeFolderEnumerationService};
    use crate::playback_events::{FakeEventEmitter, NoopEventEmitter};
    use crate::playback_service::FakePlaybackService;

    fn noop_events() -> Arc<dyn PlaybackEventEmitter> {
        Arc::new(NoopEventEmitter)
    }

    fn fake_events() -> (Arc<FakeEventEmitter>, Arc<dyn PlaybackEventEmitter>) {
        let inner = Arc::new(FakeEventEmitter::new());
        let erased = inner.clone() as Arc<dyn PlaybackEventEmitter>;
        (inner, erased)
    }

    fn noop_device() -> FakeAudioDeviceService {
        FakeAudioDeviceService::new()
    }

    fn noop_enum() -> FakeFolderEnumerationService {
        FakeFolderEnumerationService::new()
    }

    fn noop_active() -> ActiveEnumerations {
        ActiveEnumerations::new()
    }

    fn health_envelope() -> CommandEnvelope {
        CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "health".to_string(),
            payload: serde_json::json!({}),
        }
    }

    // ── Health command tests ──────────────────────────────────────────

    #[test]
    fn health_command_round_trip() {
        let mut service = FakePlaybackService::new();

        let response = dispatch(
            health_envelope(),
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(response.ok);
        let data = response.data.expect("should have data");
        assert_eq!(data, serde_json::json!({"ready": true}));
        assert!(response.error.is_none());
    }

    #[test]
    fn unknown_version_rejected() {
        let mut service = FakePlaybackService::new();

        for bad_version in [0, 99] {
            let envelope = CommandEnvelope {
                version: bad_version,
                command: "health".to_string(),
                payload: serde_json::json!({}),
            };

            let response = dispatch(
                envelope,
                &mut service,
                &mut noop_device(),
                &mut noop_enum(),
                &noop_active(),
                &noop_events(),
            );

            assert!(!response.ok);
            let error = response.error.expect("should have error");
            assert_eq!(error.category, "Unsupported");
            assert!(error.message.contains(&bad_version.to_string()));
            assert_eq!(error.diagnostic_code, "command.version");
        }
    }

    #[test]
    fn unknown_command_rejected() {
        let mut service = FakePlaybackService::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "nonexistent".to_string(),
            payload: serde_json::json!({}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(!response.ok);
        let error = response.error.expect("should have error");
        assert_eq!(error.category, "Unsupported");
        assert!(error.message.contains("nonexistent"));
        assert_eq!(error.diagnostic_code, "command.unknown");
    }

    #[test]
    fn invalid_payload_rejected() {
        let mut service = FakePlaybackService::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "health".to_string(),
            payload: serde_json::json!("not_an_object"),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(!response.ok);
        let error = response.error.expect("should have error");
        assert_eq!(error.category, "InvalidInput");
        assert_eq!(error.diagnostic_code, "command.payload");
    }

    #[test]
    fn application_error_maps_to_boundary_error() {
        let adapter_error = std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "/Users/alice/secret/file.wav",
        );
        let app_error = ApplicationError::new(
            ErrorCategory::PermissionDenied,
            DiagnosticContext::new(DiagnosticCode::BrowserRead),
            adapter_error,
        );

        let boundary = from_application_error(&app_error);

        assert_eq!(boundary.category, "PermissionDenied");
        assert_eq!(boundary.message, "PulseSeek could not access that item.");
        assert_eq!(boundary.diagnostic_code, "browser.read");
        assert!(!boundary.message.contains("alice"));
        assert!(!boundary.message.contains("secret"));
    }

    // ── Play command tests ────────────────────────────────────────────

    #[test]
    fn play_command_dispatches_to_service() {
        let mut service = FakePlaybackService::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "play".to_string(),
            payload: serde_json::json!({"path": "/music/track.wav"}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(response.ok, "play should succeed");
        assert!(response.error.is_none());
        assert_eq!(service.play_call_count, 1);
        assert_eq!(service.last_play_path, Some("/music/track.wav".to_string()));
    }

    #[test]
    fn play_command_invalid_payload_rejected() {
        let mut service = FakePlaybackService::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "play".to_string(),
            payload: serde_json::json!("not_an_object"),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(!response.ok);
        let error = response.error.expect("should have error");
        assert_eq!(error.category, "InvalidInput");
        assert_eq!(error.diagnostic_code, "command.payload");
        assert_eq!(service.play_call_count, 0, "service should not be called");
    }

    #[test]
    fn play_command_missing_path_rejected() {
        let mut service = FakePlaybackService::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "play".to_string(),
            payload: serde_json::json!({}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(!response.ok);
        let error = response.error.expect("should have error");
        assert_eq!(error.category, "InvalidInput");
        assert_eq!(service.play_call_count, 0, "service should not be called");
    }

    #[test]
    fn set_playback_mode_dispatches_and_returns_confirmed_mode() {
        let mut service = FakePlaybackService::new();
        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "set_playback_mode".to_string(),
            payload: serde_json::json!({"mode": "loop-current"}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(response.ok);
        assert_eq!(service.mode, PlaybackMode::LoopCurrent);
        assert_eq!(response.data.unwrap()["mode"], "loop-current");
    }

    #[test]
    fn set_playback_mode_rejects_unknown_mode() {
        let mut service = FakePlaybackService::new();
        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "set_playback_mode".to_string(),
            payload: serde_json::json!({"mode": "invalid"}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(!response.ok);
        assert_eq!(response.error.unwrap().diagnostic_code, "command.mode");
        assert_eq!(service.mode, PlaybackMode::OneShot);
    }

    #[test]
    fn set_playback_mode_maps_service_failure() {
        let mut service = FakePlaybackService::new();
        service.fail_with = Some(ErrorCategory::Unavailable);
        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "set_playback_mode".to_string(),
            payload: serde_json::json!({"mode": "random"}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(!response.ok);
        assert_eq!(response.error.unwrap().category, "Unavailable");
        assert_eq!(service.mode, PlaybackMode::OneShot);
    }

    // ── Pause command tests ───────────────────────────────────────────

    #[test]
    fn pause_command_dispatches_to_service() {
        let mut service = FakePlaybackService::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "pause".to_string(),
            payload: serde_json::json!({}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(response.ok);
        assert_eq!(service.pause_call_count, 1);
    }

    #[test]
    fn pause_command_invalid_payload_rejected() {
        let mut service = FakePlaybackService::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "pause".to_string(),
            payload: serde_json::json!("invalid"),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(!response.ok);
        assert_eq!(response.error.unwrap().category, "InvalidInput");
        assert_eq!(service.pause_call_count, 0);
    }

    // ── Resume command tests ──────────────────────────────────────────

    #[test]
    fn resume_command_dispatches_to_service() {
        let mut service = FakePlaybackService::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "resume".to_string(),
            payload: serde_json::json!({}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(response.ok);
        assert_eq!(service.resume_call_count, 1);
    }

    #[test]
    fn resume_command_invalid_payload_rejected() {
        let mut service = FakePlaybackService::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "resume".to_string(),
            payload: serde_json::json!("invalid"),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(!response.ok);
        assert_eq!(service.resume_call_count, 0);
    }

    // ── Stop command tests ────────────────────────────────────────────

    #[test]
    fn stop_command_dispatches_to_service() {
        let mut service = FakePlaybackService::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "stop".to_string(),
            payload: serde_json::json!({}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(response.ok);
        assert_eq!(service.stop_call_count, 1);
    }

    #[test]
    fn stop_command_invalid_payload_rejected() {
        let mut service = FakePlaybackService::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "stop".to_string(),
            payload: serde_json::json!("invalid"),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(!response.ok);
        assert_eq!(service.stop_call_count, 0);
    }

    // ── Seek command tests ────────────────────────────────────────────

    #[test]
    fn seek_command_dispatches_to_service() {
        let mut service = FakePlaybackService::new();
        service.seek_result = Some(45000);

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "seek".to_string(),
            payload: serde_json::json!({"position_ms": 45000}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(response.ok);
        assert_eq!(service.seek_call_count, 1);
        assert_eq!(service.last_seek_position, Some(45000));
        let data = response.data.expect("should have data");
        assert_eq!(data, serde_json::json!({"position_ms": 45000}));
    }

    #[test]
    fn seek_command_invalid_payload_rejected() {
        let mut service = FakePlaybackService::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "seek".to_string(),
            payload: serde_json::json!("not_an_object"),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(!response.ok);
        assert_eq!(service.seek_call_count, 0);
    }

    #[test]
    fn seek_command_missing_position_rejected() {
        let mut service = FakePlaybackService::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "seek".to_string(),
            payload: serde_json::json!({}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(!response.ok);
        assert_eq!(service.seek_call_count, 0);
    }

    // ── Volume command tests ──────────────────────────────────────────

    #[test]
    fn volume_command_dispatches_to_service() {
        let mut service = FakePlaybackService::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "volume".to_string(),
            payload: serde_json::json!({"gain": 0.75, "muted": false}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(response.ok);
        assert_eq!(service.set_volume_call_count, 1);
        assert!((service.last_volume_gain.unwrap() - 0.75).abs() < f64::EPSILON);
        assert_eq!(service.last_volume_muted, Some(false));
    }

    #[test]
    fn volume_command_muted_state_dispatches_to_service() {
        let mut service = FakePlaybackService::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "volume".to_string(),
            payload: serde_json::json!({"gain": 0.0, "muted": true}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(response.ok);
        assert_eq!(service.set_volume_call_count, 1);
        assert_eq!(service.last_volume_muted, Some(true));
    }

    #[test]
    fn volume_command_invalid_payload_rejected() {
        let mut service = FakePlaybackService::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "volume".to_string(),
            payload: serde_json::json!("not_an_object"),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(!response.ok);
        assert_eq!(service.set_volume_call_count, 0);
    }

    #[test]
    fn volume_command_missing_fields_rejected() {
        let mut service = FakePlaybackService::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "volume".to_string(),
            payload: serde_json::json!({}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(!response.ok);
        assert_eq!(service.set_volume_call_count, 0);
    }

    // ── Service error propagation tests ───────────────────────────────

    #[test]
    fn play_service_error_maps_to_boundary_error() {
        let mut service = FakePlaybackService::new();
        service.fail_with = Some(ErrorCategory::Unavailable);

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "play".to_string(),
            payload: serde_json::json!({"path": "/music/track.wav"}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(!response.ok);
        let error = response.error.expect("should have error");
        assert_eq!(error.category, "Unavailable");
        assert_eq!(error.diagnostic_code, "playback.control");
    }

    #[test]
    fn pause_service_error_maps_to_boundary_error() {
        let mut service = FakePlaybackService::new();
        service.fail_with = Some(ErrorCategory::Conflict);

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "pause".to_string(),
            payload: serde_json::json!({}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(!response.ok);
        let error = response.error.expect("should have error");
        assert_eq!(error.category, "Conflict");
    }

    #[test]
    fn resume_service_error_maps_to_boundary_error() {
        let mut service = FakePlaybackService::new();
        service.fail_with = Some(ErrorCategory::Unavailable);

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "resume".to_string(),
            payload: serde_json::json!({}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(!response.ok);
        let error = response.error.expect("should have error");
        assert_eq!(error.category, "Unavailable");
    }

    #[test]
    fn stop_service_error_maps_to_boundary_error() {
        let mut service = FakePlaybackService::new();
        service.fail_with = Some(ErrorCategory::NotFound);

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "stop".to_string(),
            payload: serde_json::json!({}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(!response.ok);
        let error = response.error.expect("should have error");
        assert_eq!(error.category, "NotFound");
    }

    #[test]
    fn seek_service_error_maps_to_boundary_error() {
        let mut service = FakePlaybackService::new();
        service.fail_with = Some(ErrorCategory::InvalidInput);

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "seek".to_string(),
            payload: serde_json::json!({"position_ms": 99999}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(!response.ok);
        let error = response.error.expect("should have error");
        assert_eq!(error.category, "InvalidInput");
    }

    #[test]
    fn volume_service_error_maps_to_boundary_error() {
        let mut service = FakePlaybackService::new();
        service.fail_with = Some(ErrorCategory::Unsupported);

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "volume".to_string(),
            payload: serde_json::json!({"gain": 1.0, "muted": false}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(!response.ok);
        let error = response.error.expect("should have error");
        assert_eq!(error.category, "Unsupported");
    }

    // ── Event emission tests ─────────────────────────────────────────

    #[test]
    fn play_emits_playing_state_event() {
        let mut service = FakePlaybackService::new();
        let (events_inner, events) = fake_events();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "play".to_string(),
            payload: serde_json::json!({"path": "/music/track.wav"}),
        };

        let _response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &events,
        );

        let recorded = events_inner.recorded_events();
        assert_eq!(recorded.len(), 1, "play should emit one event");
        assert_eq!(recorded[0].event, "playback:state-changed");
        let payload: serde_json::Value =
            serde_json::from_value(recorded[0].payload.clone()).unwrap();
        assert_eq!(payload["state"], "playing");
    }

    #[test]
    fn play_emits_event_only_on_success() {
        let mut service = FakePlaybackService::new();
        service.fail_with = Some(ErrorCategory::NotFound);
        let (events_inner, events) = fake_events();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "play".to_string(),
            payload: serde_json::json!({"path": "/music/missing.wav"}),
        };

        let _response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &events,
        );

        assert_eq!(events_inner.event_count(), 0, "no event on error");
    }

    #[test]
    fn pause_emits_paused_state_event() {
        let mut service = FakePlaybackService::new();
        let (events_inner, events) = fake_events();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "pause".to_string(),
            payload: serde_json::json!({}),
        };

        let _response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &events,
        );

        let recorded = events_inner.recorded_events();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].event, "playback:state-changed");
        let payload: serde_json::Value =
            serde_json::from_value(recorded[0].payload.clone()).unwrap();
        assert_eq!(payload["state"], "paused");
    }

    #[test]
    fn resume_emits_playing_state_event() {
        let mut service = FakePlaybackService::new();
        let (events_inner, events) = fake_events();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "resume".to_string(),
            payload: serde_json::json!({}),
        };

        let _response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &events,
        );

        let recorded = events_inner.recorded_events();
        assert_eq!(recorded.len(), 1);
        let payload: serde_json::Value =
            serde_json::from_value(recorded[0].payload.clone()).unwrap();
        assert_eq!(payload["state"], "playing");
    }

    #[test]
    fn stop_emits_stopped_state_event() {
        let mut service = FakePlaybackService::new();
        let (events_inner, events) = fake_events();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "stop".to_string(),
            payload: serde_json::json!({}),
        };

        let _response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &events,
        );

        let recorded = events_inner.recorded_events();
        assert_eq!(recorded.len(), 1);
        let payload: serde_json::Value =
            serde_json::from_value(recorded[0].payload.clone()).unwrap();
        assert_eq!(payload["state"], "stopped");
    }

    #[test]
    fn seek_does_not_emit_state_event() {
        let mut service = FakePlaybackService::new();
        let (events_inner, events) = fake_events();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "seek".to_string(),
            payload: serde_json::json!({"position_ms": 45000}),
        };

        let _response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &events,
        );

        assert_eq!(events_inner.event_count(), 0, "seek should not emit state event");
    }

    #[test]
    fn volume_does_not_emit_state_event() {
        let mut service = FakePlaybackService::new();
        let (events_inner, events) = fake_events();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "volume".to_string(),
            payload: serde_json::json!({"gain": 0.5, "muted": false}),
        };

        let _response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &events,
        );

        assert_eq!(events_inner.event_count(), 0, "volume should not emit state event");
    }

    #[test]
    fn state_event_has_versioned_envelope() {
        let mut service = FakePlaybackService::new();
        let (events_inner, events) = fake_events();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "play".to_string(),
            payload: serde_json::json!({"path": "/music/track.wav"}),
        };

        let _response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_active(),
            &events,
        );

        let recorded = events_inner.recorded_events();
        assert_eq!(recorded[0].version, crate::playback_events::CURRENT_EVENT_VERSION);
    }

    // ── Device command tests ─────────────────────────────────────────

    #[test]
    fn list_devices_command_returns_devices() {
        let mut service = FakePlaybackService::new();
        let mut device_service = FakeAudioDeviceService::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "list_devices".to_string(),
            payload: serde_json::json!({}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut device_service,
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(response.ok);
        let data = response.data.expect("should have data");
        let devices: Vec<DeviceInfoData> = serde_json::from_value(data["devices"].clone()).unwrap();
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].id, "default");
        assert_eq!(devices[0].name, "Default Output");
    }

    #[test]
    fn list_devices_command_invalid_payload_rejected() {
        let mut service = FakePlaybackService::new();
        let mut device_service = FakeAudioDeviceService::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "list_devices".to_string(),
            payload: serde_json::json!("not_an_object"),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut device_service,
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(!response.ok);
        assert_eq!(response.error.unwrap().category, "InvalidInput");
    }

    #[test]
    fn list_devices_service_error_maps_to_boundary() {
        let mut service = FakePlaybackService::new();
        let mut device_service = FakeAudioDeviceService::new();
        device_service.fail_list = true;

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "list_devices".to_string(),
            payload: serde_json::json!({}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut device_service,
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(!response.ok);
        let error = response.error.unwrap();
        assert_eq!(error.category, "Unavailable");
        assert_eq!(error.diagnostic_code, "audio.output");
    }

    #[test]
    fn current_device_command_returns_none_when_no_device() {
        let mut service = FakePlaybackService::new();
        let mut device_service = FakeAudioDeviceService::new();
        device_service.current = None;

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "current_device".to_string(),
            payload: serde_json::json!({}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut device_service,
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(response.ok);
        let data = response.data.expect("should have data");
        assert!(data["device"].is_null());
    }

    #[test]
    fn current_device_command_returns_device_when_set() {
        let mut service = FakePlaybackService::new();
        let mut device_service = FakeAudioDeviceService::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "current_device".to_string(),
            payload: serde_json::json!({}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut device_service,
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(response.ok);
        let data = response.data.expect("should have data");
        assert_eq!(data["device"]["id"], "default");
        assert_eq!(data["device"]["name"], "Default Output");
    }

    #[test]
    fn current_device_service_error_maps_to_boundary() {
        let mut service = FakePlaybackService::new();
        let mut device_service = FakeAudioDeviceService::new();
        device_service.fail_current = true;

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "current_device".to_string(),
            payload: serde_json::json!({}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut device_service,
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(!response.ok);
        assert_eq!(response.error.unwrap().category, "Unavailable");
    }

    #[test]
    fn select_device_command_dispatches_to_service() {
        let mut service = FakePlaybackService::new();
        let mut device_service = FakeAudioDeviceService::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "select_device".to_string(),
            payload: serde_json::json!({"device_id": "hdmi"}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut device_service,
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(response.ok);
        assert_eq!(device_service.select_call_count, 1);
        assert_eq!(device_service.last_select_id, Some("hdmi".to_string()));
        assert_eq!(device_service.current.as_ref().map(|d| d.id.as_str()), Some("hdmi"));
    }

    #[test]
    fn select_device_command_invalid_payload_rejected() {
        let mut service = FakePlaybackService::new();
        let mut device_service = FakeAudioDeviceService::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "select_device".to_string(),
            payload: serde_json::json!("not_an_object"),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut device_service,
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(!response.ok);
        assert_eq!(device_service.select_call_count, 0);
    }

    #[test]
    fn select_device_command_missing_id_rejected() {
        let mut service = FakePlaybackService::new();
        let mut device_service = FakeAudioDeviceService::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "select_device".to_string(),
            payload: serde_json::json!({}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut device_service,
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(!response.ok);
        assert_eq!(device_service.select_call_count, 0);
    }

    #[test]
    fn select_device_service_error_maps_to_boundary() {
        let mut service = FakePlaybackService::new();
        let mut device_service = FakeAudioDeviceService::new();
        device_service.fail_select = true;

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "select_device".to_string(),
            payload: serde_json::json!({"device_id": "hdmi"}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut device_service,
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(!response.ok);
        assert_eq!(response.error.unwrap().category, "Unavailable");
    }

    #[test]
    fn select_device_emits_device_lost_event() {
        let mut service = FakePlaybackService::new();
        let mut device_service = FakeAudioDeviceService::new();
        device_service.device_lost = true;
        let (events_inner, events) = fake_events();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "select_device".to_string(),
            payload: serde_json::json!({"device_id": "hdmi"}),
        };

        let _response = dispatch(
            envelope,
            &mut service,
            &mut device_service,
            &mut noop_enum(),
            &noop_active(),
            &events,
        );

        let recorded = events_inner.recorded_events();
        assert_eq!(recorded.len(), 1, "should emit device-lost event");
        assert_eq!(recorded[0].event, "audio:device-lost");
        let payload: serde_json::Value =
            serde_json::from_value(recorded[0].payload.clone()).unwrap();
        assert_eq!(payload["previous_device_id"], "hdmi");
    }

    #[test]
    fn select_device_does_not_emit_device_lost_when_not_lost() {
        let mut service = FakePlaybackService::new();
        let mut device_service = FakeAudioDeviceService::new();
        device_service.device_lost = false;
        let (events_inner, events) = fake_events();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "select_device".to_string(),
            payload: serde_json::json!({"device_id": "hdmi"}),
        };

        let _response = dispatch(
            envelope,
            &mut service,
            &mut device_service,
            &mut noop_enum(),
            &noop_active(),
            &events,
        );

        assert_eq!(events_inner.event_count(), 0, "should not emit event when device not lost");
    }

    #[test]
    fn unknown_device_command_rejected() {
        let mut service = FakePlaybackService::new();
        let mut device_service = FakeAudioDeviceService::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "unknown_device_op".to_string(),
            payload: serde_json::json!({}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut device_service,
            &mut noop_enum(),
            &noop_active(),
            &noop_events(),
        );

        assert!(!response.ok);
        assert_eq!(response.error.unwrap().category, "Unsupported");
    }

    // ── Enumeration command tests ─────────────────────────────────────

    #[test]
    fn start_enumeration_returns_session_id() {
        let mut service = FakePlaybackService::new();
        let mut device_service = FakeAudioDeviceService::new();
        let mut enum_service = FakeFolderEnumerationService::new();
        let active = ActiveEnumerations::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "start_enumeration".to_string(),
            payload: serde_json::json!({"path": "/music", "batch_size": 50}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut device_service,
            &mut enum_service,
            &active,
            &noop_events(),
        );

        assert!(response.ok);
        assert_eq!(enum_service.start_call_count, 1);
        assert_eq!(enum_service.last_path, Some("/music".to_string()));
        let data = response.data.expect("should have data");
        assert_eq!(data["session_id"], "test-session-001");
    }

    #[test]
    fn start_enumeration_defaults_batch_size() {
        let mut service = FakePlaybackService::new();
        let mut device_service = FakeAudioDeviceService::new();
        let mut enum_service = FakeFolderEnumerationService::new();
        let active = ActiveEnumerations::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "start_enumeration".to_string(),
            payload: serde_json::json!({"path": "/music"}),
        };

        let _response = dispatch(
            envelope,
            &mut service,
            &mut device_service,
            &mut enum_service,
            &active,
            &noop_events(),
        );

        assert_eq!(enum_service.last_batch_size, Some(100), "default batch size should be 100");
    }

    #[test]
    fn start_enumeration_invalid_payload_rejected() {
        let mut service = FakePlaybackService::new();
        let mut device_service = FakeAudioDeviceService::new();
        let mut enum_service = FakeFolderEnumerationService::new();
        let active = ActiveEnumerations::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "start_enumeration".to_string(),
            payload: serde_json::json!("not_an_object"),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut device_service,
            &mut enum_service,
            &active,
            &noop_events(),
        );

        assert!(!response.ok);
        assert_eq!(enum_service.start_call_count, 0);
    }

    #[test]
    fn start_enumeration_missing_path_rejected() {
        let mut service = FakePlaybackService::new();
        let mut device_service = FakeAudioDeviceService::new();
        let mut enum_service = FakeFolderEnumerationService::new();
        let active = ActiveEnumerations::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "start_enumeration".to_string(),
            payload: serde_json::json!({}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut device_service,
            &mut enum_service,
            &active,
            &noop_events(),
        );

        assert!(!response.ok);
        assert_eq!(enum_service.start_call_count, 0);
    }

    #[test]
    fn start_enumeration_service_error_maps_to_boundary() {
        let mut service = FakePlaybackService::new();
        let mut device_service = FakeAudioDeviceService::new();
        let mut enum_service = FakeFolderEnumerationService::new();
        enum_service.fail_start = true;
        let active = ActiveEnumerations::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "start_enumeration".to_string(),
            payload: serde_json::json!({"path": "/music"}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut device_service,
            &mut enum_service,
            &active,
            &noop_events(),
        );

        assert!(!response.ok);
        assert_eq!(response.error.unwrap().category, "Unavailable");
    }

    #[test]
    fn cancel_enumeration_cancels_session() {
        let mut service = FakePlaybackService::new();
        let mut device_service = FakeAudioDeviceService::new();
        let mut enum_service = FakeFolderEnumerationService::new();
        let active = ActiveEnumerations::new();
        let flag = active.register("session-1");

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "cancel_enumeration".to_string(),
            payload: serde_json::json!({"session_id": "session-1"}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut device_service,
            &mut enum_service,
            &active,
            &noop_events(),
        );

        assert!(response.ok);
        assert!(flag.load(std::sync::atomic::Ordering::Acquire), "session should be cancelled");
    }

    #[test]
    fn cancel_enumeration_unknown_session_idempotent() {
        let mut service = FakePlaybackService::new();
        let mut device_service = FakeAudioDeviceService::new();
        let mut enum_service = FakeFolderEnumerationService::new();
        let active = ActiveEnumerations::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "cancel_enumeration".to_string(),
            payload: serde_json::json!({"session_id": "nonexistent"}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut device_service,
            &mut enum_service,
            &active,
            &noop_events(),
        );

        assert!(response.ok, "cancelling unknown session should succeed");
    }

    #[test]
    fn cancel_enumeration_invalid_payload_rejected() {
        let mut service = FakePlaybackService::new();
        let mut device_service = FakeAudioDeviceService::new();
        let mut enum_service = FakeFolderEnumerationService::new();
        let active = ActiveEnumerations::new();

        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "cancel_enumeration".to_string(),
            payload: serde_json::json!("not_an_object"),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut device_service,
            &mut enum_service,
            &active,
            &noop_events(),
        );

        assert!(!response.ok);
        assert_eq!(response.error.unwrap().category, "InvalidInput");
    }

    // ── Folder picker command tests ────────────────────────────────────

    #[test]
    fn handle_pick_folder_returns_path() {
        let picker = FakeFolderPicker::returning(Some("/music/library"));

        let response = handle_pick_folder(&picker);

        assert!(response.ok);
        let data: PickFolderResponse = serde_json::from_value(response.data.unwrap()).unwrap();
        assert_eq!(data.path, Some("/music/library".to_string()));
    }

    #[test]
    fn handle_pick_folder_returns_none_when_cancelled() {
        let picker = FakeFolderPicker::returning(None);

        let response = handle_pick_folder(&picker);

        assert!(response.ok);
        let data: PickFolderResponse = serde_json::from_value(response.data.unwrap()).unwrap();
        assert_eq!(data.path, None);
    }

    #[test]
    fn handle_pick_folder_maps_service_error() {
        let picker = FakeFolderPicker::failing();

        let response = handle_pick_folder(&picker);

        assert!(!response.ok);
        let error = response.error.unwrap();
        assert_eq!(error.category, "PermissionDenied");
        assert_eq!(error.diagnostic_code, "browser.read");
    }
}
