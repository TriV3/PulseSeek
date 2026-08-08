use std::cmp::Ordering;
use std::fmt;
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct EntryId(String);

impl EntryId {
    pub fn new(path: &str) -> Self {
        Self(path.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EntryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let filename = Path::new(&self.0).file_name().and_then(|n| n.to_str()).unwrap_or(&self.0);
        write!(f, "{}", filename)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccessError {
    PermissionDenied,
    NotFound,
    Other(String),
}

impl fmt::Display for AccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PermissionDenied => write!(f, "permission denied"),
            Self::NotFound => write!(f, "not found"),
            Self::Other(reason) => write!(f, "{}", reason),
        }
    }
}

impl std::error::Error for AccessError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FolderEntry {
    pub id: EntryId,
    pub name: String,
    /// Shallow child-directory probe result. `None` means unknown.
    pub has_subfolders: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayableFileEntry {
    pub id: EntryId,
    pub name: String,
    pub metadata: Option<PlayableFileMetadata>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayableFileMetadata {
    pub duration_ms: Option<u64>,
    pub size_bytes: Option<u64>,
    pub modified_at_ms: Option<u64>,
    pub channels: Option<u16>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u32>,
    pub codec: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnsupportedFileEntry {
    pub id: EntryId,
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InaccessibleEntry {
    pub id: EntryId,
    pub name: String,
    pub reason: AccessError,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrowserEntry {
    Folder(FolderEntry),
    PlayableFile(PlayableFileEntry),
    UnsupportedFile(UnsupportedFileEntry),
    Inaccessible(InaccessibleEntry),
}

impl BrowserEntry {
    pub fn id(&self) -> &EntryId {
        match self {
            Self::Folder(e) => &e.id,
            Self::PlayableFile(e) => &e.id,
            Self::UnsupportedFile(e) => &e.id,
            Self::Inaccessible(e) => &e.id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Folder(e) => &e.name,
            Self::PlayableFile(e) => &e.name,
            Self::UnsupportedFile(e) => &e.name,
            Self::Inaccessible(e) => &e.name,
        }
    }

    fn variant_rank(&self) -> u8 {
        match self {
            Self::Folder(_) => 0,
            Self::PlayableFile(_) => 1,
            Self::UnsupportedFile(_) => 2,
            Self::Inaccessible(_) => 3,
        }
    }
}

impl Ord for BrowserEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        let rank_cmp = self.variant_rank().cmp(&other.variant_rank());
        if rank_cmp != Ordering::Equal {
            return rank_cmp;
        }
        let name_cmp = self.name().to_lowercase().cmp(&other.name().to_lowercase());
        if name_cmp != Ordering::Equal {
            return name_cmp;
        }
        self.id().cmp(other.id())
    }
}

impl PartialOrd for BrowserEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests;
