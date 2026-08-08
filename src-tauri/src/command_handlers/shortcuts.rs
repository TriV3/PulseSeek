use serde::Serialize;
use serde_json::Value;

use crate::command_envelope::types::{
    LoadShortcutsRequest, LoadShortcutsResponse, ResetShortcutsRequest, ResetShortcutsResponse,
    SaveShortcutsRequest, SaveShortcutsResponse,
};
use crate::command_envelope::{from_application_error, CommandResponse};
use crate::command_handlers::parse_payload;
use crate::shortcut_mappings_service::{ShortcutMappingsData, ShortcutMappingsService};

pub(crate) fn handle(
    command: &str,
    payload: Value,
    service: &dyn ShortcutMappingsService,
) -> CommandResponse {
    match command {
        "load_shortcuts" => {
            let _request: LoadShortcutsRequest = match parse_payload(command, payload) {
                Ok(request) => request,
                Err(response) => return response,
            };
            respond(service.load(), LoadShortcutsResponse::from)
        },
        "save_shortcuts" => {
            let request: SaveShortcutsRequest = match parse_payload(command, payload) {
                Ok(request) => request,
                Err(response) => return response,
            };
            respond(service.save(request.mappings), SaveShortcutsResponse::from)
        },
        "reset_shortcuts" => {
            let _request: ResetShortcutsRequest = match parse_payload(command, payload) {
                Ok(request) => request,
                Err(response) => return response,
            };
            respond(service.reset(), ResetShortcutsResponse::from)
        },
        _ => unreachable!("unhandled shortcut command: {command}"),
    }
}

fn respond<T: Serialize>(
    result: Result<ShortcutMappingsData, pulseseek_domain::error::ApplicationError>,
    convert: impl FnOnce(ShortcutMappingsData) -> T,
) -> CommandResponse {
    match result {
        Ok(data) => CommandResponse::ok(serde_json::to_value(convert(data)).unwrap()),
        Err(error) => CommandResponse::err(from_application_error(&error)),
    }
}

macro_rules! response_from_profile {
    ($response:ty) => {
        impl From<ShortcutMappingsData> for $response {
            fn from(data: ShortcutMappingsData) -> Self {
                Self {
                    mappings: data.mappings,
                    unavailable_action_ids: data.unavailable_action_ids,
                }
            }
        }
    };
}

response_from_profile!(LoadShortcutsResponse);
response_from_profile!(SaveShortcutsResponse);
response_from_profile!(ResetShortcutsResponse);
