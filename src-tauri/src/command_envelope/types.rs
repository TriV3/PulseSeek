use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::audio_device_service::DeviceInfoData;

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

#[derive(Debug, Deserialize)]
pub struct PickFolderRequest {}

#[derive(Debug, Serialize, Deserialize)]
pub struct PickFolderResponse {
    pub path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListBrowserRootsRequest {}

#[derive(Debug, Serialize)]
pub struct ListBrowserRootsResponse {
    pub roots: Vec<crate::folder_enumeration_service::BrowserRootData>,
}

#[derive(Debug, Deserialize)]
pub struct StartEnumerationRequest {
    pub path: String,
    pub batch_size: Option<u64>,
    #[serde(default)]
    pub show_unsupported: bool,
    #[serde(default)]
    pub recursive: bool,
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

#[derive(Debug, Deserialize)]
pub struct MoveToTrashRequest {
    pub paths: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MoveToTrashResponse {
    pub results: Vec<MoveToTrashItemResult>,
}

#[derive(Debug, Deserialize)]
pub struct RenameFileRequest {
    pub path: String,
    pub new_name: String,
}

#[derive(Debug, Deserialize)]
pub struct StartMoveFilesRequest {
    pub paths: Vec<String>,
    pub target_dir: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StartMoveFilesResponse {
    pub session_id: String,
}

#[derive(Debug, Deserialize)]
pub struct CancelMoveFilesRequest {
    pub session_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CancelMoveFilesResponse {}

#[derive(Debug, Deserialize)]
pub struct StartCopyFilesRequest {
    pub paths: Vec<String>,
    pub target_dir: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StartCopyFilesResponse {
    pub session_id: String,
}

#[derive(Debug, Deserialize)]
pub struct CancelCopyFilesRequest {
    pub session_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CancelCopyFilesResponse {}

#[derive(Debug, Deserialize)]
pub struct RevealFileRequest {
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RevealFileResponse {}

#[derive(Debug, Deserialize)]
pub struct OpenWithRequest {
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenWithResponse {}

#[derive(Debug, Deserialize)]
pub struct DragOutRequest {
    pub paths: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DragOutResponse {}

#[derive(Debug, Serialize, Deserialize)]
pub struct RenameFileResponse {
    pub old_path: String,
    pub new_path: String,
    /// True when the renamed file is the currently playing file (FR-FM-009).
    pub was_playing: bool,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct MoveToTrashItemResult {
    pub path: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic_code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListRecentFoldersRequest {}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListRecentFoldersResponse {
    pub folders: Vec<crate::recent_folders_service::RecentFolderData>,
}

#[derive(Debug, Deserialize)]
pub struct RecordRecentFolderRequest {
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecordRecentFolderResponse {}

#[derive(Debug, Deserialize)]
pub struct ClearRecentFoldersRequest {}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClearRecentFoldersResponse {}

impl CommandResponse {
    pub fn ok(data: Value) -> Self {
        Self { version: CURRENT_COMMAND_VERSION, ok: true, data: Some(data), error: None }
    }

    pub fn err(error: BoundaryError) -> Self {
        Self { version: CURRENT_COMMAND_VERSION, ok: false, data: None, error: Some(error) }
    }
}
