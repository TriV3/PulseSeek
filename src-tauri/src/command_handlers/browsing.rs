use std::path::PathBuf;
use std::sync::Arc;

use pulseseek_domain::error::ErrorContract;
use serde_json::Value;

use crate::command_envelope::types::{
    CancelEnumerationRequest, CancelEnumerationResponse, ListBrowserRootsRequest,
    ListBrowserRootsResponse, MoveToTrashItemResult, MoveToTrashRequest, MoveToTrashResponse,
    PickFolderResponse, StartEnumerationRequest, StartEnumerationResponse,
};
use crate::command_envelope::{from_application_error, CommandResponse};
use crate::command_handlers::parse_payload;
use crate::dialog_service::FolderPicker;
use crate::folder_enumeration_service::{ActiveEnumerations, FolderEnumerationService};
use crate::playback_events::PlaybackEventEmitter;
use crate::trash_service::TrashService;

/// Handles browsing commands: start_enumeration, cancel_enumeration, and
/// move_to_trash.
pub(crate) fn handle(
    command: &str,
    payload: Value,
    enum_service: &mut dyn FolderEnumerationService,
    trash_service: &dyn TrashService,
    active: &ActiveEnumerations,
    events: &Arc<dyn PlaybackEventEmitter>,
) -> CommandResponse {
    match command {
        "list_browser_roots" => {
            let _request: ListBrowserRootsRequest =
                match parse_payload("list_browser_roots", payload) {
                    Ok(request) => request,
                    Err(response) => return response,
                };
            match enum_service.list_roots() {
                Ok(roots) => CommandResponse::ok(
                    serde_json::to_value(ListBrowserRootsResponse { roots }).unwrap(),
                ),
                Err(error) => CommandResponse::err(from_application_error(&error)),
            }
        },
        "start_enumeration" => {
            let request: StartEnumerationRequest = match parse_payload("start_enumeration", payload)
            {
                Ok(request) => request,
                Err(response) => return response,
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
            let request: CancelEnumerationRequest =
                match parse_payload("cancel_enumeration", payload) {
                    Ok(request) => request,
                    Err(response) => return response,
                };
            active.cancel(&request.session_id);
            CommandResponse::ok(serde_json::to_value(CancelEnumerationResponse {}).unwrap())
        },
        "move_to_trash" => {
            let request: MoveToTrashRequest = match parse_payload("move_to_trash", payload) {
                Ok(request) => request,
                Err(response) => return response,
            };
            if request.paths.is_empty() {
                return CommandResponse::ok(
                    serde_json::to_value(MoveToTrashResponse { results: vec![] }).unwrap(),
                );
            }
            let path_bufs: Vec<PathBuf> = request.paths.iter().map(PathBuf::from).collect();
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
        _ => unreachable!("unhandled browsing command: {command}"),
    }
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
