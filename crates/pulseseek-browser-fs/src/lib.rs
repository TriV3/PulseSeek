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
///
/// Uses `std::fs::read_dir` to enumerate direct children of a folder.
/// Results are sorted using [`BrowserEntry`] ordering.
pub struct NativeFolderReader;

impl FolderReader for NativeFolderReader {
    fn read_folder(&self, path: &Path) -> Result<Vec<BrowserEntry>, FolderReadError> {
        self.read_folder_with_options(path, false)
    }
}

impl NativeFolderReader {
    /// Reads direct children and hides files that no registered decoder can open.
    ///
    /// `show_unsupported` reveals those files as `UnsupportedFile` entries. Probe
    /// results come from file content, never from the filename extension.
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
mod tests {
    use super::*;
    use pulseseek_domain::error::ErrorContract;
    use std::fs;
    use std::path::Path;

    fn create_dir(path: &Path) {
        fs::create_dir_all(path).expect("create test dir");
    }

    fn create_file(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dir");
        }
        fs::write(path, b"content").expect("create test file");
    }

    fn create_wav(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dir");
        }
        fs::write(
            path,
            include_bytes!(
                "../../pulseseek-decoder-symphonia/tests/fixtures/silent-stereo-44100.wav"
            ),
        )
        .expect("create WAV fixture");
    }

    #[test]
    fn native_reads_empty_folder() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let result = NativeFolderReader.read_folder(dir.path()).unwrap();
        assert!(result.is_empty(), "empty folder should return no entries");
    }

    #[test]
    fn native_reads_folder_with_files() {
        let dir = tempfile::tempdir().expect("create temp dir");
        create_wav(&dir.path().join("a.wav"));
        create_wav(&dir.path().join("b.wav"));

        let result = NativeFolderReader.read_folder(dir.path()).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn native_reads_file_and_audio_metadata() {
        let dir = tempfile::tempdir().expect("create temp dir");
        create_wav(&dir.path().join("audio.wav"));

        let result = NativeFolderReader.read_folder(dir.path()).unwrap();
        let BrowserEntry::PlayableFile(file) = &result[0] else {
            panic!("expected playable file");
        };
        let metadata = file.metadata.as_ref().expect("metadata should be available");

        assert!(metadata.duration_ms.is_some());
        assert!(metadata.size_bytes.is_some_and(|size| size > 0));
        assert!(metadata.modified_at_ms.is_some());
        assert_eq!(metadata.channels, Some(2));
        assert_eq!(metadata.sample_rate, Some(44_100));
        assert_eq!(metadata.bit_depth, Some(16));
        assert_eq!(metadata.codec.as_deref(), Some("PCM"));
    }

    #[test]
    fn zero_stream_values_are_unavailable() {
        let metadata = playable_file_metadata(
            Some(pulseseek_domain::decoder::StreamMetadata {
                sample_rate: 0,
                channels: 0,
                duration: Duration::from_millis(0),
                bit_depth: None,
                codec: "MP3",
            }),
            None,
        );

        assert_eq!(metadata.duration_ms, None);
        assert_eq!(metadata.channels, None);
        assert_eq!(metadata.sample_rate, None);
        assert_eq!(metadata.codec.as_deref(), Some("MP3"));
    }

    #[test]
    fn native_reads_folder_with_nested_folder() {
        let dir = tempfile::tempdir().expect("create temp dir");
        create_wav(&dir.path().join("root.wav"));
        create_dir(&dir.path().join("sub"));
        create_file(&dir.path().join("sub").join("nested.wav"));

        let result = NativeFolderReader.read_folder(dir.path()).unwrap();

        // Only direct children: root.wav + sub/
        assert_eq!(result.len(), 2, "should only return direct children");

        // sub is a folder, root.wav is a file
        let folder_count = result.iter().filter(|e| matches!(e, BrowserEntry::Folder(_))).count();
        let file_count =
            result.iter().filter(|e| matches!(e, BrowserEntry::PlayableFile(_))).count();
        assert_eq!(folder_count, 1, "should have one folder");
        assert_eq!(file_count, 1, "should have one file");
    }

    #[test]
    fn native_reads_folder_with_symlink() {
        let dir = tempfile::tempdir().expect("create temp dir");
        create_wav(&dir.path().join("target.wav"));
        let link_path = dir.path().join("link.wav");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(dir.path().join("target.wav"), &link_path)
                .expect("create symlink");
        }
        #[cfg(windows)]
        {
            // Symlinks require admin on Windows — skip this test.
            return;
        }

        let result = NativeFolderReader.read_folder(dir.path()).unwrap();
        let names: Vec<&str> = result.iter().map(|e| e.name()).collect();
        assert!(names.contains(&"link.wav"), "symlink should be listed");
        assert!(names.contains(&"target.wav"), "target should be listed");
    }

    #[test]
    fn native_reads_folder_with_unicode_names() {
        let dir = tempfile::tempdir().expect("create temp dir");
        create_wav(&dir.path().join("über-jam.wav"));
        create_wav(&dir.path().join("alpha.wav"));

        let result = NativeFolderReader.read_folder(dir.path()).unwrap();
        let names: Vec<&str> = result.iter().map(|e| e.name()).collect();
        assert!(names.contains(&"über-jam.wav"));
        assert!(names.contains(&"alpha.wav"));
    }

    #[test]
    fn native_returns_error_for_nonexistent_path() {
        let result = NativeFolderReader.read_folder(Path::new("/nonexistent/path/xyz"));
        assert!(result.is_err(), "nonexistent path should return error");
        let err = result.unwrap_err();
        assert_eq!(
            err.user_descriptor().category(),
            pulseseek_domain::error::ErrorCategory::NotFound
        );
    }

    #[test]
    fn native_returns_error_for_file_path() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let file_path = dir.path().join("file.wav");
        create_file(&file_path);

        let result = NativeFolderReader.read_folder(&file_path);
        assert!(result.is_err(), "file path should return error");
    }

    #[test]
    fn native_entries_are_sorted() {
        let dir = tempfile::tempdir().expect("create temp dir");
        create_dir(&dir.path().join("zzz_folder"));
        create_wav(&dir.path().join("aaa_file.wav"));
        create_dir(&dir.path().join("bbb_folder"));
        create_wav(&dir.path().join("ccc_file.wav"));

        let result = NativeFolderReader.read_folder(dir.path()).unwrap();
        let names: Vec<&str> = result.iter().map(|e| e.name()).collect();

        // Folders first, then files, alphabetically
        assert_eq!(names, vec!["bbb_folder", "zzz_folder", "aaa_file.wav", "ccc_file.wav"]);
    }

    #[test]
    fn native_hides_unsupported_files_by_default() {
        let dir = tempfile::tempdir().expect("create temp dir");
        create_wav(&dir.path().join("audio.bin"));
        create_file(&dir.path().join("notes.txt"));

        let result = NativeFolderReader.read_folder(dir.path()).unwrap();

        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], BrowserEntry::PlayableFile(_)));
        assert_eq!(result[0].name(), "audio.bin");
    }

    #[test]
    fn native_can_show_unsupported_files() {
        let dir = tempfile::tempdir().expect("create temp dir");
        create_wav(&dir.path().join("audio.txt"));
        create_file(&dir.path().join("notes.txt"));

        let result = NativeFolderReader.read_folder_with_options(dir.path(), true).unwrap();

        assert_eq!(result.len(), 2);
        assert!(matches!(result[0], BrowserEntry::PlayableFile(_)));
        assert!(matches!(result[1], BrowserEntry::UnsupportedFile(_)));
    }

    #[test]
    fn native_rejects_corrupt_audio_as_unsupported() {
        let dir = tempfile::tempdir().expect("create temp dir");
        create_file(&dir.path().join("corrupt.wav"));

        let result = NativeFolderReader.read_folder_with_options(dir.path(), true).unwrap();

        assert!(matches!(result[0], BrowserEntry::UnsupportedFile(_)));
    }
}
