pub mod copy_file;
pub mod drag_out;
pub mod external_actions;
pub mod move_file;
pub mod rename;
pub mod trash;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;

use pulseseek_decoder_symphonia::probe_stream_metadata;
use pulseseek_domain::browser::entry::{
    AccessError, BrowserEntry, EntryId, FolderEntry, InaccessibleEntry, PlayableFileEntry,
    PlayableFileMetadata, UnsupportedFileEntry,
};
use pulseseek_domain::browser::folder_reader::{FolderReadError, FolderReader};
use pulseseek_domain::decoder::StreamMetadata;
use pulseseek_domain::playback::position::Duration;

/// Native filesystem adapter for [`FolderReader`].
pub struct NativeFolderReader;

#[derive(Clone, Copy)]
struct RecursiveReadOptions {
    show_unsupported: bool,
    show_hidden: bool,
    batch_size: usize,
}

impl FolderReader for NativeFolderReader {
    fn read_folder(&self, path: &Path) -> Result<Vec<BrowserEntry>, FolderReadError> {
        self.read_folder_with_options(path, false)
    }
}

impl NativeFolderReader {
    /// Maximum number of verified files delivered in one interactive update.
    /// Small chunks make the first rows visible quickly without flooding the
    /// frontend with one event per file.
    const MAX_INTERACTIVE_BATCH_SIZE: usize = 16;

    /// Returns folder names and likely supported audio files without opening
    /// decoders. This keeps navigation responsive while verified metadata is
    /// collected in a second pass.
    pub fn read_folder_preview(
        &self,
        path: &Path,
        show_unsupported: bool,
    ) -> Result<Vec<BrowserEntry>, FolderReadError> {
        self.read_folder_preview_with_options(path, show_unsupported, false)
    }

    pub fn read_folder_preview_with_options(
        &self,
        path: &Path,
        show_unsupported: bool,
        show_hidden: bool,
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
                if !show_hidden && name.starts_with('.') {
                    continue;
                }
                entries.push(BrowserEntry::Folder(FolderEntry {
                    id,
                    name,
                    has_subfolders: directory_has_subfolder(&entry.path(), show_hidden),
                }));
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

    /// Streams folder names and extension-recognized audio candidates in small
    /// batches without opening files. Candidate rows intentionally carry no
    /// metadata; the parallel verification pass enriches valid files and emits
    /// an unsupported tombstone for candidates that must be removed.
    pub fn stream_folder_preview(
        &self,
        path: &Path,
        show_unsupported: bool,
        show_hidden: bool,
        batch_size: usize,
        is_cancelled: impl Fn() -> bool + Sync,
        mut on_chunk: impl FnMut(&[BrowserEntry]),
    ) -> Result<(), FolderReadError> {
        let dir_reader =
            std::fs::read_dir(path).map_err(|e| FolderReadError::from_io_error(e, path))?;
        let batch_size = batch_size.max(1);
        let mut immediate = Vec::with_capacity(batch_size);
        let mut folder_candidates = Vec::new();
        let mut emitted_entries = false;

        for entry in dir_reader {
            if is_cancelled() {
                return Ok(());
            }
            let entry = entry.map_err(|e| FolderReadError::from_io_error(e, path))?;
            let file_type =
                entry.file_type().map_err(|e| FolderReadError::from_io_error(e, &entry.path()))?;
            let name = entry.file_name().to_string_lossy().to_string();
            let child_path = entry.path();
            let path_string = child_path.to_string_lossy().to_string();
            if file_type.is_dir() {
                if !show_hidden && name.starts_with('.') {
                    continue;
                }
                folder_candidates.push((child_path, name));
            } else if likely_supported_audio(&child_path) {
                immediate.push(BrowserEntry::PlayableFile(PlayableFileEntry {
                    id: EntryId::new(&path_string),
                    name,
                    metadata: None,
                }));
            } else if show_unsupported {
                immediate.push(BrowserEntry::UnsupportedFile(UnsupportedFileEntry {
                    id: EntryId::new(&path_string),
                    name,
                }));
            }
        }

        immediate.sort();
        for chunk in immediate.chunks(batch_size) {
            if is_cancelled() {
                return Ok(());
            }
            on_chunk(chunk);
            emitted_entries = true;
        }

        if !folder_candidates.is_empty() && !is_cancelled() {
            let available_workers =
                std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
            let worker_count =
                available_workers.saturating_sub(2).clamp(1, 6).min(folder_candidates.len());
            let next_index = AtomicUsize::new(0);
            let (sender, receiver) = mpsc::channel();
            let interactive_batch_size = batch_size.min(Self::MAX_INTERACTIVE_BATCH_SIZE);
            let mut next_batch_size = interactive_batch_size.min(4);
            let mut pending = Vec::with_capacity(interactive_batch_size);

            std::thread::scope(|scope| {
                for _ in 0..worker_count {
                    let sender = sender.clone();
                    let folder_candidates = &folder_candidates;
                    let next_index = &next_index;
                    let is_cancelled = &is_cancelled;
                    scope.spawn(move || loop {
                        if is_cancelled() {
                            break;
                        }
                        let index = next_index.fetch_add(1, Ordering::Relaxed);
                        let Some((path, name)) = folder_candidates.get(index) else {
                            break;
                        };
                        let folder = BrowserEntry::Folder(FolderEntry {
                            id: EntryId::new(&path.to_string_lossy()),
                            name: name.clone(),
                            has_subfolders: directory_has_subfolder(path, show_hidden),
                        });
                        if sender.send(folder).is_err() {
                            break;
                        }
                    });
                }
                drop(sender);

                for entry in receiver {
                    if is_cancelled() {
                        break;
                    }
                    pending.push(entry);
                    if pending.len() == next_batch_size {
                        pending.sort();
                        on_chunk(&pending);
                        pending.clear();
                        emitted_entries = true;
                        next_batch_size = interactive_batch_size;
                    }
                }
            });

            if !is_cancelled() && !pending.is_empty() {
                pending.sort();
                on_chunk(&pending);
                emitted_entries = true;
            }
        }

        if !is_cancelled() && !emitted_entries {
            on_chunk(&[]);
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
                if name_str.starts_with('.') {
                    continue;
                }
                BrowserEntry::Folder(FolderEntry {
                    id: EntryId::new(&path_str),
                    name: name_str,
                    has_subfolders: directory_has_subfolder(&entry.path(), false),
                })
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

    /// Verifies direct child files concurrently and streams small result
    /// batches as soon as workers finish. Directory discovery remains a
    /// separate lightweight preview, while decoder probing and metadata I/O
    /// use a bounded pool that reserves CPU capacity for playback.
    pub fn stream_folder_files_parallel<C, E>(
        &self,
        path: &Path,
        show_unsupported: bool,
        batch_size: usize,
        is_cancelled: C,
        mut on_batch: E,
    ) -> Result<(), FolderReadError>
    where
        C: Fn() -> bool + Sync,
        E: FnMut(&[BrowserEntry]),
    {
        let dir_reader =
            std::fs::read_dir(path).map_err(|e| FolderReadError::from_io_error(e, path))?;
        let mut candidates = Vec::new();
        let mut immediate = Vec::new();

        for entry in dir_reader {
            if is_cancelled() {
                return Ok(());
            }
            let entry = entry.map_err(|e| FolderReadError::from_io_error(e, path))?;
            let file_type =
                entry.file_type().map_err(|e| FolderReadError::from_io_error(e, &entry.path()))?;
            if file_type.is_dir() {
                continue;
            }

            let child_path = entry.path();
            if likely_supported_audio(&child_path) {
                candidates.push(child_path);
            } else if show_unsupported {
                immediate.push(BrowserEntry::UnsupportedFile(UnsupportedFileEntry {
                    id: EntryId::new(&child_path.to_string_lossy()),
                    name: entry.file_name().to_string_lossy().to_string(),
                }));
            }
        }

        let interactive_batch_size = batch_size.clamp(1, Self::MAX_INTERACTIVE_BATCH_SIZE);
        immediate.sort();
        for chunk in immediate.chunks(interactive_batch_size) {
            if is_cancelled() {
                return Ok(());
            }
            on_batch(chunk);
        }

        if candidates.is_empty() || is_cancelled() {
            return Ok(());
        }

        let available_workers =
            std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
        // Keep two logical cores free for playback/UI while using enough
        // parallel probes to finish large metadata batches interactively.
        let worker_count = available_workers.saturating_sub(2).clamp(1, 6).min(candidates.len());
        let next_index = AtomicUsize::new(0);
        let (sender, receiver) = mpsc::channel();
        let mut pending = Vec::with_capacity(interactive_batch_size);
        let mut next_batch_size = interactive_batch_size.min(4);

        std::thread::scope(|scope| {
            for _ in 0..worker_count {
                let sender = sender.clone();
                let candidates = &candidates;
                let next_index = &next_index;
                let is_cancelled = &is_cancelled;
                scope.spawn(move || loop {
                    if is_cancelled() {
                        break;
                    }
                    let index = next_index.fetch_add(1, Ordering::Relaxed);
                    let Some(candidate) = candidates.get(index) else {
                        break;
                    };
                    // Even when unsupported rows are hidden, rejected preview
                    // candidates must cross the boundary as tombstones so the
                    // frontend can remove their optimistic rows.
                    if let Some(entry) = self.classify_child(candidate, true) {
                        if sender.send(entry).is_err() {
                            break;
                        }
                    }
                });
            }
            drop(sender);

            for entry in receiver {
                if is_cancelled() {
                    break;
                }
                pending.push(entry);
                if pending.len() == next_batch_size {
                    pending.sort();
                    on_batch(&pending);
                    pending.clear();
                    next_batch_size = interactive_batch_size;
                }
            }
        });

        if !is_cancelled() && !pending.is_empty() {
            pending.sort();
            on_batch(&pending);
        }
        Ok(())
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
        show_hidden: bool,
        batch_size: usize,
        is_cancelled: impl Fn() -> bool,
        on_batch: impl FnMut(&[BrowserEntry]),
    ) -> Result<(), FolderReadError> {
        self.stream_recursive_files_with_reader(
            path,
            RecursiveReadOptions { show_unsupported, show_hidden, batch_size },
            is_cancelled,
            on_batch,
            default_read_children,
        )
    }

    fn stream_recursive_files_with_reader<F, C, E>(
        &self,
        root: &Path,
        options: RecursiveReadOptions,
        is_cancelled: C,
        mut on_batch: E,
        mut read_children: F,
    ) -> Result<(), FolderReadError>
    where
        F: FnMut(&Path) -> std::io::Result<Vec<ChildListing>>,
        C: Fn() -> bool,
        E: FnMut(&[BrowserEntry]),
    {
        let batch_size = options.batch_size.max(1);
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
                if !options.show_hidden && file_name_string(&child.path).starts_with('.') {
                    continue;
                }
                root_dirs.push(child.path);
            } else if let Some(entry) = self.classify_child(&child.path, options.show_unsupported) {
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
                    if !options.show_hidden && file_name_string(&child.path).starts_with('.') {
                        continue;
                    }
                    child_dirs.push(child.path);
                } else if let Some(entry) =
                    self.classify_child(&child.path, options.show_unsupported)
                {
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
        } else if let Ok(stream_metadata) = probe_stream_metadata(path) {
            let file_metadata = std::fs::metadata(path).ok();
            Some(BrowserEntry::PlayableFile(PlayableFileEntry {
                id,
                name,
                metadata: Some(playable_file_metadata(
                    Some(stream_metadata),
                    file_metadata.as_ref(),
                )),
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

/// Looks only far enough into `path` to determine whether an expand control is
/// useful. Errors remain unknown so inaccessible folders stay expandable and
/// can surface their normal access error when selected.
fn directory_has_subfolder(path: &Path, show_hidden: bool) -> Option<bool> {
    let entries = std::fs::read_dir(path).ok()?;
    for entry in entries {
        let entry = entry.ok()?;
        if entry.file_type().ok()?.is_dir()
            && (show_hidden || !entry.file_name().to_string_lossy().starts_with('.'))
        {
            return Some(true);
        }
    }
    Some(false)
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
