use pulseseek_domain::error::{ApplicationError, ErrorContract};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

impl CommandResponse {
    pub fn ok(data: Value) -> Self {
        Self { version: CURRENT_COMMAND_VERSION, ok: true, data: Some(data), error: None }
    }

    pub fn err(error: BoundaryError) -> Self {
        Self { version: CURRENT_COMMAND_VERSION, ok: false, data: None, error: Some(error) }
    }
}

pub fn dispatch(envelope: CommandEnvelope) -> CommandResponse {
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
pub fn invoke_command(envelope: CommandEnvelope) -> CommandResponse {
    dispatch(envelope)
}

#[cfg(test)]
mod tests {
    use pulseseek_domain::error::{DiagnosticCode, DiagnosticContext, ErrorCategory};

    use super::*;

    #[test]
    fn health_command_round_trip() {
        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "health".to_string(),
            payload: serde_json::json!({}),
        };

        let response = dispatch(envelope);

        assert!(response.ok);
        let data = response.data.expect("should have data");
        assert_eq!(data, serde_json::json!({"ready": true}));
        assert!(response.error.is_none());
    }

    #[test]
    fn unknown_version_rejected() {
        for bad_version in [0, 99] {
            let envelope = CommandEnvelope {
                version: bad_version,
                command: "health".to_string(),
                payload: serde_json::json!({}),
            };

            let response = dispatch(envelope);

            assert!(!response.ok);
            let error = response.error.expect("should have error");
            assert_eq!(error.category, "Unsupported");
            assert!(error.message.contains(&bad_version.to_string()));
            assert_eq!(error.diagnostic_code, "command.version");
        }
    }

    #[test]
    fn unknown_command_rejected() {
        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "nonexistent".to_string(),
            payload: serde_json::json!({}),
        };

        let response = dispatch(envelope);

        assert!(!response.ok);
        let error = response.error.expect("should have error");
        assert_eq!(error.category, "Unsupported");
        assert!(error.message.contains("nonexistent"));
        assert_eq!(error.diagnostic_code, "command.unknown");
    }

    #[test]
    fn invalid_payload_rejected() {
        let envelope = CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "health".to_string(),
            payload: serde_json::json!("not_an_object"),
        };

        let response = dispatch(envelope);

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
}
