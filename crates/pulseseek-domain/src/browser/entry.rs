use std::cmp::Ordering;
use std::fmt;
use std::path::Path;

/// Unique identifier for a browser entry based on its filesystem path.
///
/// Two entries with the same path are considered equal. The `Display`
/// implementation shows only the filename (safe — no full path leak).
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct EntryId(String);

impl EntryId {
    /// Creates a new `EntryId` from a filesystem path string.
    pub fn new(path: &str) -> Self {
        Self(path.to_string())
    }

    /// Returns the full path string.
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

/// Reason an entry could not be accessed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccessError {
    /// The process does not have permission to read the entry.
    PermissionDenied,
    /// The entry does not exist at the expected path.
    NotFound,
    /// Another reason (e.g. filesystem error, broken symlink).
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

/// A browsable folder entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FolderEntry {
    pub id: EntryId,
    pub name: String,
}

/// A playable audio file entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayableFileEntry {
    pub id: EntryId,
    pub name: String,
    pub metadata: Option<PlayableFileMetadata>,
}

/// Available filesystem and stream metadata for a playable file.
///
/// Every field remains optional so metadata failures never make a playable
/// browser entry disappear.
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

/// An unsupported (non-playable) file entry, hidden by default.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnsupportedFileEntry {
    pub id: EntryId,
    pub name: String,
}

/// An entry that could not be accessed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InaccessibleEntry {
    pub id: EntryId,
    pub name: String,
    pub reason: AccessError,
}

/// A browsable filesystem entry.
///
/// Variants are ordered: `Folder` < `PlayableFile` < `UnsupportedFile` < `Inaccessible`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrowserEntry {
    Folder(FolderEntry),
    PlayableFile(PlayableFileEntry),
    UnsupportedFile(UnsupportedFileEntry),
    Inaccessible(InaccessibleEntry),
}

impl BrowserEntry {
    /// Returns the unique identifier for this entry.
    pub fn id(&self) -> &EntryId {
        match self {
            Self::Folder(e) => &e.id,
            Self::PlayableFile(e) => &e.id,
            Self::UnsupportedFile(e) => &e.id,
            Self::Inaccessible(e) => &e.id,
        }
    }

    /// Returns the display name of this entry.
    pub fn name(&self) -> &str {
        match self {
            Self::Folder(e) => &e.name,
            Self::PlayableFile(e) => &e.name,
            Self::UnsupportedFile(e) => &e.name,
            Self::Inaccessible(e) => &e.name,
        }
    }

    /// Returns the variant rank for ordering (lower = sorted first).
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
        // Within the same variant, compare by name (case-insensitive).
        let name_cmp = self.name().to_lowercase().cmp(&other.name().to_lowercase());
        if name_cmp != Ordering::Equal {
            return name_cmp;
        }
        // Fallback to full path comparison for stable ordering.
        self.id().cmp(other.id())
    }
}

impl PartialOrd for BrowserEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry_id(path: &str) -> EntryId {
        EntryId::new(path)
    }

    fn folder(path: &str, name: &str) -> BrowserEntry {
        BrowserEntry::Folder(FolderEntry { id: entry_id(path), name: name.to_string() })
    }

    fn playable(path: &str, name: &str) -> BrowserEntry {
        BrowserEntry::PlayableFile(PlayableFileEntry {
            id: entry_id(path),
            name: name.to_string(),
            metadata: None,
        })
    }

    fn unsupported(path: &str, name: &str) -> BrowserEntry {
        BrowserEntry::UnsupportedFile(UnsupportedFileEntry {
            id: entry_id(path),
            name: name.to_string(),
        })
    }

    fn inaccessible(path: &str, name: &str, reason: AccessError) -> BrowserEntry {
        BrowserEntry::Inaccessible(InaccessibleEntry {
            id: entry_id(path),
            name: name.to_string(),
            reason,
        })
    }

    // ── EntryId tests ─────────────────────────────────────────────────

    #[test]
    fn entry_id_display_shows_filename_only() {
        let id = entry_id("/Users/alice/Music/track.wav");
        assert_eq!(id.to_string(), "track.wav");
    }

    #[test]
    fn entry_id_display_shows_filename_for_deep_path() {
        let id = entry_id("/a/b/c/d/e/f/song.flac");
        assert_eq!(id.to_string(), "song.flac");
    }

    #[test]
    fn entry_id_display_shows_unicode_filename() {
        let id = entry_id("/music/über-jam.wav");
        assert_eq!(id.to_string(), "über-jam.wav");
    }

    #[test]
    fn entry_id_display_shows_path_when_no_filename() {
        let id = entry_id("/");
        assert_eq!(id.to_string(), "/");
    }

    #[test]
    fn entry_id_equality_by_full_path() {
        let a = entry_id("/music/track.wav");
        let b = entry_id("/music/track.wav");
        let c = entry_id("/music/other.wav");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn entry_id_ordering_by_path() {
        let a = entry_id("/a/alpha.wav");
        let b = entry_id("/b/beta.wav");
        assert!(a < b);
    }

    #[test]
    fn entry_id_as_str_returns_full_path() {
        let id = entry_id("/full/path/file.wav");
        assert_eq!(id.as_str(), "/full/path/file.wav");
    }

    // ── BrowserEntry ordering tests ───────────────────────────────────

    #[test]
    fn entry_ordering_folders_before_files() {
        let file = playable("/music/song.wav", "song.wav");
        let dir = folder("/music", "music");
        assert!(dir < file, "folders should come before files");
    }

    #[test]
    fn entry_ordering_playable_before_unsupported() {
        let file = playable("/music/song.wav", "song.wav");
        let unsup = unsupported("/music/readme.txt", "readme.txt");
        assert!(file < unsup, "playable files should come before unsupported");
    }

    #[test]
    fn entry_ordering_unsupported_before_inaccessible() {
        let unsup = unsupported("/music/readme.txt", "readme.txt");
        let inacc = inaccessible("/music/corrupt", "corrupt", AccessError::PermissionDenied);
        assert!(unsup < inacc, "unsupported should come before inaccessible");
    }

    #[test]
    fn entry_ordering_alphabetical_within_kind() {
        let a = folder("/music/alpha", "alpha");
        let b = folder("/music/beta", "beta");
        assert!(a < b);
    }

    #[test]
    fn entry_ordering_case_insensitive() {
        let a = folder("/music/ALPHA", "ALPHA");
        let b = folder("/music/beta", "beta");
        assert!(a < b, "case-insensitive: ALPHA < beta");
    }

    #[test]
    fn entry_ordering_unicode_names() {
        let a = folder("/music/anna", "anna");
        let b = folder("/music/éclair", "éclair");
        assert!(a < b, "unicode names should sort");
    }

    #[test]
    fn entry_ordering_mixed_kinds() {
        let mut entries = [
            playable("/music/zebra.wav", "zebra.wav"),
            folder("/music/alpha", "alpha"),
            inaccessible("/bad", "bad", AccessError::NotFound),
            unsupported("/notes.txt", "notes.txt"),
            folder("/music/beta", "beta"),
        ];
        entries.sort();

        assert_eq!(entries[0].name(), "alpha", "first folder");
        assert_eq!(entries[1].name(), "beta", "second folder");
        assert_eq!(entries[2].name(), "zebra.wav", "first playable");
        assert_eq!(entries[3].name(), "notes.txt", "first unsupported");
        assert_eq!(entries[4].name(), "bad", "first inaccessible");
    }

    // ── AccessError tests ─────────────────────────────────────────────

    #[test]
    fn access_error_display() {
        assert_eq!(AccessError::PermissionDenied.to_string(), "permission denied");
        assert_eq!(AccessError::NotFound.to_string(), "not found");
        assert_eq!(AccessError::Other("broken symlink".to_string()).to_string(), "broken symlink");
    }

    // ── BrowserEntry accessor tests ───────────────────────────────────

    #[test]
    fn browser_entry_id_accessor() {
        let e = playable("/music/song.wav", "song.wav");
        assert_eq!(e.id().as_str(), "/music/song.wav");
    }

    #[test]
    fn browser_entry_name_accessor() {
        let e = folder("/music", "My Music");
        assert_eq!(e.name(), "My Music");
    }

    #[test]
    fn playable_entry_keeps_partial_metadata() {
        let entry = BrowserEntry::PlayableFile(PlayableFileEntry {
            id: entry_id("/music/song.mp3"),
            name: "song.mp3".to_string(),
            metadata: Some(PlayableFileMetadata {
                duration_ms: Some(61_000),
                size_bytes: Some(1_572_864),
                modified_at_ms: None,
                channels: Some(2),
                sample_rate: Some(44_100),
                bit_depth: None,
                codec: Some("MP3".to_string()),
            }),
        });

        let BrowserEntry::PlayableFile(file) = entry else {
            panic!("expected playable file");
        };
        let metadata = file.metadata.expect("metadata should be present");
        assert_eq!(metadata.duration_ms, Some(61_000));
        assert_eq!(metadata.bit_depth, None);
        assert_eq!(metadata.codec.as_deref(), Some("MP3"));
    }

    #[test]
    fn browser_entry_id_accessor_for_all_variants() {
        let entries = [
            folder("/a", "a"),
            playable("/b.wav", "b.wav"),
            unsupported("/c.txt", "c.txt"),
            inaccessible("/d", "d", AccessError::PermissionDenied),
        ];
        for entry in &entries {
            assert!(!entry.id().as_str().is_empty(), "id should not be empty");
            assert!(!entry.name().is_empty(), "name should not be empty");
        }
    }
}
