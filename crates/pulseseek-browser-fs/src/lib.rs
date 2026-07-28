use std::path::Path;

use pulseseek_domain::browser::entry::{BrowserEntry, EntryId, FolderEntry, PlayableFileEntry};
use pulseseek_domain::browser::folder_reader::{FolderReadError, FolderReader};

/// Native filesystem adapter for [`FolderReader`].
///
/// Uses `std::fs::read_dir` to enumerate direct children of a folder.
/// Results are sorted using [`BrowserEntry`] ordering.
pub struct NativeFolderReader;

impl FolderReader for NativeFolderReader {
    fn read_folder(&self, path: &Path) -> Result<Vec<BrowserEntry>, FolderReadError> {
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
                // Files and symlinks to files are treated as playable.
                // Decoder probing is handled in a later PR.
                BrowserEntry::PlayableFile(PlayableFileEntry {
                    id: EntryId::new(&path_str),
                    name: name_str,
                })
            };
            entries.push(browser_entry);
        }

        entries.sort();
        Ok(entries)
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

    #[test]
    fn native_reads_empty_folder() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let result = NativeFolderReader.read_folder(dir.path()).unwrap();
        assert!(result.is_empty(), "empty folder should return no entries");
    }

    #[test]
    fn native_reads_folder_with_files() {
        let dir = tempfile::tempdir().expect("create temp dir");
        create_file(&dir.path().join("a.wav"));
        create_file(&dir.path().join("b.wav"));

        let result = NativeFolderReader.read_folder(dir.path()).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn native_reads_folder_with_nested_folder() {
        let dir = tempfile::tempdir().expect("create temp dir");
        create_file(&dir.path().join("root.wav"));
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
        create_file(&dir.path().join("target.wav"));
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
        create_file(&dir.path().join("über-jam.wav"));
        create_file(&dir.path().join("alpha.wav"));

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
        create_file(&dir.path().join("aaa_file.wav"));
        create_dir(&dir.path().join("bbb_folder"));
        create_file(&dir.path().join("ccc_file.wav"));

        let result = NativeFolderReader.read_folder(dir.path()).unwrap();
        let names: Vec<&str> = result.iter().map(|e| e.name()).collect();

        // Folders first, then files, alphabetically
        assert_eq!(names, vec!["bbb_folder", "zzz_folder", "aaa_file.wav", "ccc_file.wav"]);
    }
}
