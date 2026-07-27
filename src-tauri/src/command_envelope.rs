use pulseseek_domain::error::{ApplicationError, ErrorContract};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

impl CommandResponse {
    pub fn ok(data: Value) -> Self {
        Self { version: CURRENT_COMMAND_VERSION, ok: true, data: Some(data), error: None }
    }

    pub fn err(error: BoundaryError) -> Self {
        Self { version: CURRENT_COMMAND_VERSION, ok: false, data: None, error: Some(error) }
    }
}

pub fn dispatch(envelope: CommandEnvelope, service: &mut dyn PlaybackService) -> CommandResponse {
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
                Ok(()) => CommandResponse::ok(serde_json::to_value(PlayResponse {}).unwrap()),
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
                Ok(()) => CommandResponse::ok(serde_json::to_value(PauseResponse {}).unwrap()),
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
                Ok(()) => CommandResponse::ok(serde_json::to_value(ResumeResponse {}).unwrap()),
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
                Ok(()) => CommandResponse::ok(serde_json::to_value(StopResponse {}).unwrap()),
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

/// Tauri command: dispatches a versioned command envelope and returns a
/// versioned response. This is the single entry point for all frontend
/// commands.
#[tauri::command]
pub fn invoke_command(
    envelope: CommandEnvelope,
    state: tauri::State<'_, std::sync::Mutex<Box<dyn PlaybackService>>>,
) -> CommandResponse {
    let mut service = state.lock().expect("playback service lock poisoned");
    dispatch(envelope, &mut **service)
}

#[cfg(test)]
mod tests {
    use pulseseek_domain::error::{DiagnosticCode, DiagnosticContext, ErrorCategory};

    use super::*;
    use crate::playback_service::FakePlaybackService;

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

        let response = dispatch(health_envelope(), &mut service);

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

            let response = dispatch(envelope, &mut service);

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

        let response = dispatch(envelope, &mut service);

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

        let response = dispatch(envelope, &mut service);

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

        let response = dispatch(envelope, &mut service);

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

        let response = dispatch(envelope, &mut service);

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

        let response = dispatch(envelope, &mut service);

        assert!(!response.ok);
        let error = response.error.expect("should have error");
        assert_eq!(error.category, "InvalidInput");
        assert_eq!(service.play_call_count, 0, "service should not be called");
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

        let response = dispatch(envelope, &mut service);

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

        let response = dispatch(envelope, &mut service);

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

        let response = dispatch(envelope, &mut service);

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

        let response = dispatch(envelope, &mut service);

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

        let response = dispatch(envelope, &mut service);

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

        let response = dispatch(envelope, &mut service);

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

        let response = dispatch(envelope, &mut service);

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

        let response = dispatch(envelope, &mut service);

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

        let response = dispatch(envelope, &mut service);

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

        let response = dispatch(envelope, &mut service);

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

        let response = dispatch(envelope, &mut service);

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

        let response = dispatch(envelope, &mut service);

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

        let response = dispatch(envelope, &mut service);

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

        let response = dispatch(envelope, &mut service);

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

        let response = dispatch(envelope, &mut service);

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

        let response = dispatch(envelope, &mut service);

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

        let response = dispatch(envelope, &mut service);

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

        let response = dispatch(envelope, &mut service);

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

        let response = dispatch(envelope, &mut service);

        assert!(!response.ok);
        let error = response.error.expect("should have error");
        assert_eq!(error.category, "Unsupported");
    }
}
