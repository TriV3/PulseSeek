pub mod trash;

use std::path::Path;

use pulseseek_decoder_symphonia::registry::DecoderRegistry;
use pulseseek_domain::browser::entry::{
    BrowserEntry, EntryId, FolderEntry, PlayableFileEntry, PlayableFileMetadata,
    UnsupportedFileEntry,
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
    pub fn read_folder_with_options(
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
            let name = entry.file_name();
            let name_str = name.to_string_lossy().to_string();
            let path_str = entry.path().to_string_lossy().to_string();

            let browser_entry = if file_type.is_dir() {
                BrowserEntry::Folder(FolderEntry { id: EntryId::new(&path_str), name: name_str })
            } else {
                let id = EntryId::new(&path_str);
                if let Ok(mut decoder) = DecoderRegistry::open(entry.path()) {
                    let stream_metadata = decoder.metadata().ok();
                    let file_metadata = entry.metadata().ok();
                    let metadata = playable_file_metadata(stream_metadata, file_metadata.as_ref());
                    BrowserEntry::PlayableFile(PlayableFileEntry {
                        id,
                        name: name_str,
                        metadata: Some(metadata),
                    })
                } else if show_unsupported {
                    BrowserEntry::UnsupportedFile(UnsupportedFileEntry { id, name: name_str })
                } else {
                    continue;
                }
            };
            entries.push(browser_entry);
        }

        entries.sort();
        Ok(entries)
    }
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
