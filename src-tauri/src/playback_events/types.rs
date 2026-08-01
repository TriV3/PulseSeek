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
