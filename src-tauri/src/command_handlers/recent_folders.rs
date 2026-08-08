use serde_json::Value;

use crate::command_envelope::types::{
    ChangeFolderBookmarkRequest, ChangeFolderBookmarkResponse, ClearRecentFoldersRequest,
    ClearRecentFoldersResponse, ListFolderBookmarksRequest, ListFolderBookmarksResponse,
    ListRecentFoldersRequest, ListRecentFoldersResponse, RecordRecentFolderRequest,
    RecordRecentFolderResponse,
};
use crate::command_envelope::{from_application_error, CommandResponse};
use crate::command_handlers::parse_payload;
use crate::recent_folders_service::RecentFoldersService;

/// Handles recent-folder commands: list_recent_folders, record_recent_folder,
/// and clear_recent_folders (FR-BR-011).
pub(crate) fn handle(
    command: &str,
    payload: Value,
    service: &dyn RecentFoldersService,
) -> CommandResponse {
    match command {
        "list_recent_folders" => {
            let _request: ListRecentFoldersRequest =
                match parse_payload("list_recent_folders", payload) {
                    Ok(request) => request,
                    Err(response) => return response,
                };
            match service.list_recent_folders() {
                Ok(folders) => CommandResponse::ok(
                    serde_json::to_value(ListRecentFoldersResponse { folders }).unwrap(),
                ),
                Err(error) => CommandResponse::err(from_application_error(&error)),
            }
        },
        "record_recent_folder" => {
            let request: RecordRecentFolderRequest =
                match parse_payload("record_recent_folder", payload) {
                    Ok(request) => request,
                    Err(response) => return response,
                };
            match service.record_recent_folder(&request.path) {
                Ok(()) => CommandResponse::ok(
                    serde_json::to_value(RecordRecentFolderResponse {}).unwrap(),
                ),
                Err(error) => CommandResponse::err(from_application_error(&error)),
            }
        },
        "clear_recent_folders" => {
            let _request: ClearRecentFoldersRequest =
                match parse_payload("clear_recent_folders", payload) {
                    Ok(request) => request,
                    Err(response) => return response,
                };
            match service.clear_recent_folders() {
                Ok(()) => CommandResponse::ok(
                    serde_json::to_value(ClearRecentFoldersResponse {}).unwrap(),
                ),
                Err(error) => CommandResponse::err(from_application_error(&error)),
            }
        },
        "list_folder_bookmarks" => {
            let _request: ListFolderBookmarksRequest =
                match parse_payload("list_folder_bookmarks", payload) {
                    Ok(request) => request,
                    Err(response) => return response,
                };
            match service.list_bookmarks() {
                Ok(bookmarks) => CommandResponse::ok(
                    serde_json::to_value(ListFolderBookmarksResponse { bookmarks }).unwrap(),
                ),
                Err(error) => CommandResponse::err(from_application_error(&error)),
            }
        },
        "add_folder_bookmark" | "remove_folder_bookmark" => {
            let request: ChangeFolderBookmarkRequest = match parse_payload(command, payload) {
                Ok(request) => request,
                Err(response) => return response,
            };
            let result = if command == "add_folder_bookmark" {
                service.add_bookmark(&request.path)
            } else {
                service.remove_bookmark(&request.path)
            };
            match result {
                Ok(()) => CommandResponse::ok(
                    serde_json::to_value(ChangeFolderBookmarkResponse {}).unwrap(),
                ),
                Err(error) => CommandResponse::err(from_application_error(&error)),
            }
        },
        _ => unreachable!("unhandled recent folder command: {command}"),
    }
}
