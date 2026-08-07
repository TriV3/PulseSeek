use serde::{Deserialize, Serialize};

/// Payload for [`EVENT_STATE_CHANGED`](super::EVENT_STATE_CHANGED).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateChangedPayload {
    pub state: String,
}

/// Payload for [`EVENT_POSITION`](super::EVENT_POSITION).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PositionPayload {
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
}

/// Payload for [`EVENT_DEVICE_LOST`](super::EVENT_DEVICE_LOST).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceLostPayload {
    pub previous_device_id: String,
}

/// Serializable browser entry sent to the frontend.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrowserEntryData {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub metadata: Option<PlayableFileMetadataData>,
}

/// Serializable playable-file metadata sent to the frontend.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayableFileMetadataData {
    pub duration_ms: Option<u64>,
    pub size_bytes: Option<u64>,
    pub modified_at_ms: Option<u64>,
    pub channels: Option<u16>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u32>,
    pub codec: Option<String>,
}

/// Payload for [`EVENT_FOLDER_CHUNK`](super::EVENT_FOLDER_CHUNK).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FolderChunkPayload {
    pub session_id: String,
    pub entries: Vec<BrowserEntryData>,
    #[serde(default)]
    pub folders_done: bool,
    pub done: bool,
}

/// Payload for [`EVENT_FILE_CHANGE`](super::EVENT_FILE_CHANGE).
///
/// Signals that the watched folder changed and the frontend should re-read it.
/// The payload carries the watched path so a stale listener can ignore changes
/// that no longer belong to the open folder.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileChangePayload {
    pub path: String,
}
/// One file's outcome in a move batch, reported through
/// [`EVENT_MOVE_PROGRESS`](super::EVENT_MOVE_PROGRESS). Successful and failed
/// targets are reported separately so the UI can summarize each group.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MoveItemResultData {
    /// Source path of the file before the move.
    pub path: String,
    /// Full path after a successful move; absent when the file failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_path: Option<String>,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic_code: Option<String>,
}
/// Payload for [`EVENT_MOVE_PROGRESS`](super::EVENT_MOVE_PROGRESS).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MoveProgressPayload {
    pub session_id: String,
    /// Number of files processed so far, including failures.
    pub completed: usize,
    /// Total number of files in the batch.
    pub total: usize,
    /// True when the batch finished (all files processed or cancelled).
    pub done: bool,
    /// Per-file results in batch order. Only populated on the final event
    /// (`done == true`); intermediate events carry an empty list so large
    /// batches stay O(N) over the event channel.
    pub results: Vec<MoveItemResultData>,
}
/// One file's outcome in a copy batch, reported through
/// [`EVENT_COPY_PROGRESS`](super::EVENT_COPY_PROGRESS). Successful and failed
/// targets are reported separately so the UI can summarize each group.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CopyItemResultData {
    /// Source path of the file being copied.
    pub path: String,
    /// Full path of the created copy; absent when the file failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_path: Option<String>,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic_code: Option<String>,
}
/// Payload for [`EVENT_COPY_PROGRESS`](super::EVENT_COPY_PROGRESS).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CopyProgressPayload {
    pub session_id: String,
    /// Number of files processed so far, including failures.
    pub completed: usize,
    /// Total number of files in the batch.
    pub total: usize,
    /// True when the batch finished (all files processed or cancelled).
    pub done: bool,
    /// Per-file results in batch order. Only populated on the final event
    /// (`done == true`); intermediate events carry an empty list so large
    /// batches stay O(N) over the event channel.
    pub results: Vec<CopyItemResultData>,
}
