pub mod move_file;
pub mod rename;
pub mod trash;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use pulseseek_decoder_symphonia::registry::DecoderRegistry;
use pulseseek_domain::browser::entry::{
    AccessError, BrowserEntry, EntryId, FolderEntry, InaccessibleEntry, PlayableFileEntry,
    PlayableFileMetadata, UnsupportedFileEntry,
};
use pulseseek_domain::browser::folder_reader::{FolderReadError, FolderReader};
use pulseseek_domain::decoder::StreamMetadata;
use pulseseek_domain::playback::position::Duration;

/// Native filesystem adapter for [`FolderReader`].
pub struct NativeFolderReader;

impl FolderReader for NativeFolderReader {
    fn read_folder(&self, path: &Path) -> Result<Vec<BrowserEntry>, FolderReadError> {
        self.read_folder_with_options(path, false)
    }
}

impl NativeFolderReader {
    /// Returns folder names and likely supported audio files without opening
    /// decoders. This keeps navigation responsive while verified metadata is
    /// collected in a second pass.
    pub fn read_folder_preview(
        &self,
        path: &Path,
        show_unsupported: bool,
    ) -> Result<Vec<BrowserEntry>, FolderReadError> {
        let dir_reader =
            std::fs::read_dir(path).map_err(|e| FolderReadError::from_io_error(e, path))?;
        let mut entries = Vec::new();
        for entry in dir_reader {
            let entry = entry.map_err(|e| FolderReadError::from_io_error(e, path))?;
            let file_type =
                entry.file_type().map_err(|e| FolderReadError::from_io_error(e, &entry.path()))?;
            let name = entry.file_name().to_string_lossy().to_string();
            let path_string = entry.path().to_string_lossy().to_string();
            let id = EntryId::new(&path_string);
            if file_type.is_dir() {
                entries.push(BrowserEntry::Folder(FolderEntry { id, name }));
            } else if likely_supported_audio(&entry.path()) {
                entries.push(BrowserEntry::PlayableFile(PlayableFileEntry {
                    id,
                    name,
                    metadata: None,
                }));
            } else if show_unsupported {
                entries.push(BrowserEntry::UnsupportedFile(UnsupportedFileEntry { id, name }));
            }
        }
        entries.sort();
        Ok(entries)
    }

    /// Streams folder names in small batches without opening files. Files are
    /// intentionally omitted: only the decoder verification pass may expose
    /// a playable file, so a non-audio file with an audio-looking extension
    /// can never flash in the File List.
    pub fn stream_folder_preview(
        &self,
        path: &Path,
        _show_unsupported: bool,
        batch_size: usize,
        is_cancelled: impl Fn() -> bool,
        mut on_chunk: impl FnMut(&[BrowserEntry]),
    ) -> Result<(), FolderReadError> {
        let dir_reader =
            std::fs::read_dir(path).map_err(|e| FolderReadError::from_io_error(e, path))?;
        let mut entries = Vec::with_capacity(batch_size);
        let mut emitted_entries = false;

        for entry in dir_reader {
            if is_cancelled() {
                return Ok(());
            }
            let entry = entry.map_err(|e| FolderReadError::from_io_error(e, path))?;
            let file_type =
                entry.file_type().map_err(|e| FolderReadError::from_io_error(e, &entry.path()))?;
            if file_type.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                let path_string = entry.path().to_string_lossy().to_string();
                entries.push(BrowserEntry::Folder(FolderEntry {
                    id: EntryId::new(&path_string),
                    name,
                }));
            }
            if entries.len() == batch_size {
                entries.sort();
                on_chunk(&entries);
                entries.clear();
                emitted_entries = true;
            }
        }

        if !is_cancelled() && (!entries.is_empty() || !emitted_entries) {
            entries.sort();
            on_chunk(&entries);
        }
        Ok(())
    }

    pub fn read_folder_with_options(
        &self,
        path: &Path,
        show_unsupported: bool,
    ) -> Result<Vec<BrowserEntry>, FolderReadError> {
        self.read_folder_with_options_cancellable(path, show_unsupported, || false)
    }

    pub fn read_folder_with_options_cancellable(
        &self,
        path: &Path,
        show_unsupported: bool,
        is_cancelled: impl Fn() -> bool,
    ) -> Result<Vec<BrowserEntry>, FolderReadError> {
        let dir_reader =
            std::fs::read_dir(path).map_err(|e| FolderReadError::from_io_error(e, path))?;

        let mut entries = Vec::new();
        for entry in dir_reader {
            if is_cancelled() {
                break;
            }
            let entry = entry.map_err(|e| FolderReadError::from_io_error(e, path))?;
            let file_type =
                entry.file_type().map_err(|e| FolderReadError::from_io_error(e, &entry.path()))?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy().to_string();
            let path_str = entry.path().to_string_lossy().to_string();

            let browser_entry = if file_type.is_dir() {
                BrowserEntry::Folder(FolderEntry { id: EntryId::new(&path_str), name: name_str })
            } else {
                match self.classify_child(&entry.path(), show_unsupported) {
                    Some(entry) => entry,
                    None => continue,
                }
            };
            entries.push(browser_entry);
        }

        entries.sort();
        Ok(entries)
    }

    /// Recursively walks `path` and streams playable files (and, when
    /// requested, unsupported files) in deterministic depth-first order: each
    /// directory emits its own files before descending into sorted
    /// subdirectories. Directory symlinks are followed but cycles are broken
    /// by remembering canonicalized directories, so a symlink loop terminates
    /// without duplicating entries. Unreadable or vanished subdirectories
    /// become `Inaccessible` boundary entries instead of aborting the walk;
    /// only a failure to read the root propagates as an error.
    pub fn stream_recursive_files(
        &self,
        path: &Path,
        show_unsupported: bool,
        batch_size: usize,
        is_cancelled: impl Fn() -> bool,
        on_batch: impl FnMut(&[BrowserEntry]),
    ) -> Result<(), FolderReadError> {
        self.stream_recursive_files_with_reader(
            path,
            show_unsupported,
            batch_size,
            is_cancelled,
            on_batch,
            default_read_children,
        )
    }

    fn stream_recursive_files_with_reader<F, C, E>(
        &self,
        root: &Path,
        show_unsupported: bool,
        batch_size: usize,
        is_cancelled: C,
        mut on_batch: E,
        mut read_children: F,
    ) -> Result<(), FolderReadError>
    where
        F: FnMut(&Path) -> std::io::Result<Vec<ChildListing>>,
        C: Fn() -> bool,
        E: FnMut(&[BrowserEntry]),
    {
        let batch_size = batch_size.max(1);
        let root_children =
            read_children(root).map_err(|error| FolderReadError::from_io_error(error, root))?;

        let mut visited = HashSet::new();
        if let Ok(canonical) = root.canonicalize() {
            visited.insert(canonical);
        }

        let mut pending: Vec<BrowserEntry> = Vec::with_capacity(batch_size);
        let mut flush = |pending: &mut Vec<BrowserEntry>| {
            if pending.is_empty() {
                return;
            }
            on_batch(pending);
            pending.clear();
        };

        // Process the root directory's own files first (sorted), then its
        // subdirectories in sorted order.
        let mut dir_stack: Vec<PathBuf> = Vec::new();
        let mut root_files = Vec::new();
        let mut root_dirs = Vec::new();
        for child in root_children {
            if is_cancelled() {
                flush(&mut pending);
                return Ok(());
            }
            if is_directory(&child) {
                root_dirs.push(child.path);
            } else if let Some(entry) = self.classify_child(&child.path, show_unsupported) {
                root_files.push(entry);
            }
        }
        root_files.sort();
        for entry in root_files {
            if is_cancelled() {
                break;
            }
            pending.push(entry);
            if pending.len() >= batch_size {
                flush(&mut pending);
            }
        }
        sort_child_directories(&mut root_dirs);
        dir_stack.extend(root_dirs.into_iter().rev());

        while let Some(dir) = dir_stack.pop() {
            if is_cancelled() {
                break;
            }
            let child_listings = match read_children(&dir) {
                Ok(listings) => listings,
                Err(error) => {
                    pending.push(BrowserEntry::Inaccessible(InaccessibleEntry {
                        id: EntryId::new(&dir.to_string_lossy()),
                        name: file_name_string(&dir),
                        reason: access_error_from_io(&error),
                    }));
                    if pending.len() >= batch_size {
                        flush(&mut pending);
                    }
                    continue;
                },
            };
            let mut child_dirs = Vec::new();
            let mut child_files = Vec::new();
            for child in child_listings {
                if is_cancelled() {
                    break;
                }
                if is_directory(&child) {
                    child_dirs.push(child.path);
                } else if let Some(entry) = self.classify_child(&child.path, show_unsupported) {
                    child_files.push(entry);
                }
            }
            child_files.sort();
            for entry in child_files {
                if is_cancelled() {
                    break;
                }
                pending.push(entry);
                if pending.len() >= batch_size {
                    flush(&mut pending);
                }
            }
            sort_child_directories(&mut child_dirs);
            for child_dir in child_dirs.into_iter().rev() {
                if is_cancelled() {
                    break;
                }
                let canonical = match child_dir.canonicalize() {
                    Ok(canonical) => canonical,
                    Err(_) => child_dir.clone(),
                };
                if visited.insert(canonical) {
                    dir_stack.push(child_dir);
                }
            }
        }

        if !is_cancelled() {
            flush(&mut pending);
        }
        Ok(())
    }

    /// Classifies a candidate file path into a playable, unsupported, or
    /// skipped browser entry. Directories are classified by callers.
    fn classify_child(&self, path: &Path, show_unsupported: bool) -> Option<BrowserEntry> {
        let id = EntryId::new(&path.to_string_lossy());
        let name = file_name_string(path);
        if !likely_supported_audio(path) {
            if show_unsupported {
                Some(BrowserEntry::UnsupportedFile(UnsupportedFileEntry { id, name }))
            } else {
                None
            }
        } else if let Ok(mut decoder) = DecoderRegistry::open(path) {
            let stream_metadata = decoder.metadata().ok();
            let file_metadata = std::fs::metadata(path).ok();
            Some(BrowserEntry::PlayableFile(PlayableFileEntry {
                id,
                name,
                metadata: Some(playable_file_metadata(stream_metadata, file_metadata.as_ref())),
            }))
        } else if show_unsupported {
            Some(BrowserEntry::UnsupportedFile(UnsupportedFileEntry { id, name }))
        } else {
            None
        }
    }
}

/// A single child discovered while listing a directory.
pub(crate) struct ChildListing {
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_symlink: bool,
}

/// Reads the direct children of a directory as [`ChildListing`] values.
fn default_read_children(dir: &Path) -> std::io::Result<Vec<ChildListing>> {
    let mut children = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        children.push(ChildListing {
            path: entry.path(),
            is_dir: file_type.is_dir(),
            is_symlink: file_type.is_symlink(),
        });
    }
    Ok(children)
}

/// Returns whether a listing is a directory to descend into, following
/// symlinks so linked folders participate in the recursive view.
fn is_directory(child: &ChildListing) -> bool {
    child.is_dir || (child.is_symlink && child.path.is_dir())
}

/// Sorts child directory paths case-insensitively for deterministic descent.
fn sort_child_directories(dirs: &mut [PathBuf]) {
    dirs.sort_by(|left, right| {
        left.file_name().and_then(|name| name.to_str()).unwrap_or_default().to_lowercase().cmp(
            &right.file_name().and_then(|name| name.to_str()).unwrap_or_default().to_lowercase(),
        )
    });
}

/// Extracts the last path component as a string, falling back to the whole
/// path when the component is not valid UTF-8.
fn file_name_string(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

/// Maps an `io::Error` to the domain access error used for boundary entries.
fn access_error_from_io(error: &std::io::Error) -> AccessError {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => AccessError::PermissionDenied,
        std::io::ErrorKind::NotFound => AccessError::NotFound,
        _ => AccessError::Other(error.to_string()),
    }
}

fn likely_supported_audio(path: &Path) -> bool {
    path.extension().and_then(|extension| extension.to_str()).is_some_and(|extension| {
        matches!(extension.to_ascii_lowercase().as_str(), "mp3" | "flac" | "wav" | "wave")
    })
}

fn playable_file_metadata(
    stream_metadata: Option<StreamMetadata>,
    file_metadata: Option<&std::fs::Metadata>,
) -> PlayableFileMetadata {
    PlayableFileMetadata {
        duration_ms: stream_metadata.as_ref().and_then(|metadata| {
            if let Duration::Known(duration) = metadata.duration {
                (duration.as_millis() > 0).then_some(duration.as_millis())
            } else {
                None
            }
        }),
        size_bytes: file_metadata.map(std::fs::Metadata::len),
        modified_at_ms: file_metadata
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| modified.duration_since(std::time::SystemTime::UNIX_EPOCH).ok())
            .and_then(|duration| u64::try_from(duration.as_millis()).ok()),
        channels: stream_metadata
            .as_ref()
            .and_then(|metadata| (metadata.channels > 0).then_some(metadata.channels)),
        sample_rate: stream_metadata
            .as_ref()
            .and_then(|metadata| (metadata.sample_rate > 0).then_some(metadata.sample_rate)),
        bit_depth: stream_metadata.as_ref().and_then(|metadata| metadata.bit_depth),
        codec: stream_metadata.as_ref().map(|metadata| metadata.codec.to_string()),
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
