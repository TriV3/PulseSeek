//! Per-domain command handlers behind the versioned command envelope.

pub(crate) mod browsing;
pub(crate) mod device;
pub(crate) mod playback;
pub(crate) mod recent_folders;
pub(crate) mod waveform;

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::command_envelope::{BoundaryError, CommandResponse};

/// Parses a command payload into a typed request, mapping parse failures to
/// the standard `command.payload` boundary error. The `command` name is
/// included in the message so callers can identify which command failed.
pub(crate) fn parse_payload<T: DeserializeOwned>(
    command: &str,
    payload: Value,
) -> Result<T, CommandResponse> {
    serde_json::from_value(payload).map_err(|error| {
        CommandResponse::err(BoundaryError {
            category: "InvalidInput".to_string(),
            message: format!("Invalid {command} command payload: {error}"),
            diagnostic_code: "command.payload".to_string(),
        })
    })
}
