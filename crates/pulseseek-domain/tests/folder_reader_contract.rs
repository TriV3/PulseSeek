use std::path::Path;

use pulseseek_domain::browser::entry::{BrowserEntry, EntryId, FolderEntry, PlayableFileEntry};
use pulseseek_domain::browser::folder_reader::{FolderReadError, FolderReader};
use pulseseek_domain::error::{ErrorCategory, ErrorContract};

/// Fake implementation of [`FolderReader`] for contract tests.
struct FakeFolderReader {
    entries: Vec<BrowserEntry>,
    fail_with: Option<ErrorCategory>,
}

impl FolderReader for FakeFolderReader {
    fn read_folder(&self, path: &Path) -> Result<Vec<BrowserEntry>, FolderReadError> {
        if let Some(category) = self.fail_with {
            let io_err = match category {
                ErrorCategory::PermissionDenied => {
                    std::io::Error::new(std::io::ErrorKind::PermissionDenied, "fake denied")
                },
                ErrorCategory::NotFound => {
                    std::io::Error::new(std::io::ErrorKind::NotFound, "fake not found")
                },
                _ => std::io::Error::other("fake error"),
            };
            return Err(FolderReadError::from_io_error(io_err, path));
        }
        let mut entries = self.entries.clone();
        entries.sort();
        Ok(entries)
    }
}

fn entry(path: &str, name: &str) -> BrowserEntry {
    BrowserEntry::PlayableFile(PlayableFileEntry {
        id: EntryId::new(path),
        name: name.to_string(),
        metadata: None,
    })
}

fn folder_entry(path: &str, name: &str) -> BrowserEntry {
    BrowserEntry::Folder(FolderEntry {
        id: EntryId::new(path),
        name: name.to_string(),
        has_subfolders: None,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────

#[test]
fn fake_returns_entries() {
    let reader = FakeFolderReader {
        entries: vec![entry("/a.wav", "a.wav"), entry("/b.wav", "b.wav")],
        fail_with: None,
    };
    let result = reader.read_folder(Path::new("/")).unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn fake_empty_folder() {
    let reader = FakeFolderReader { entries: vec![], fail_with: None };
    let result = reader.read_folder(Path::new("/empty")).unwrap();
    assert!(result.is_empty());
}

#[test]
fn fake_returns_error() {
    let reader =
        FakeFolderReader { entries: vec![], fail_with: Some(ErrorCategory::PermissionDenied) };
    let err = reader.read_folder(Path::new("/restricted")).unwrap_err();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::PermissionDenied);
}

#[test]
fn fake_entries_are_sorted() {
    let reader = FakeFolderReader {
        entries: vec![entry("/z.wav", "z.wav"), folder_entry("/a", "a"), entry("/b.wav", "b.wav")],
        fail_with: None,
    };
    let result = reader.read_folder(Path::new("/")).unwrap();
    assert_eq!(result[0].name(), "a", "folders first");
    assert_eq!(result[1].name(), "b.wav", "then files alphabetically");
    assert_eq!(result[2].name(), "z.wav");
}

#[test]
fn fake_unicode_entries() {
    let reader = FakeFolderReader {
        entries: vec![entry("/über.wav", "über.wav"), entry("/alpha.wav", "alpha.wav")],
        fail_with: None,
    };
    let result = reader.read_folder(Path::new("/")).unwrap();
    // "alpha" < "über" with simple string comparison
    assert_eq!(result[0].name(), "alpha.wav");
    assert_eq!(result[1].name(), "über.wav");
}

#[test]
fn fake_nested_not_recursive() {
    let reader = FakeFolderReader {
        entries: vec![
            folder_entry("/parent/child", "child"),
            entry("/parent/file.wav", "file.wav"),
        ],
        fail_with: None,
    };
    let result = reader.read_folder(Path::new("/parent")).unwrap();
    assert_eq!(result.len(), 2, "only direct children");
    // None of the children of /parent/child should appear
    for entry in &result {
        assert!(
            !entry.id().as_str().contains("/parent/child/"),
            "no recursive entries: {}",
            entry.id()
        );
    }
}

#[test]
fn folder_read_error_implements_error_contract() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "test");
    let err = FolderReadError::from_io_error(io_err, Path::new("/test"));
    let desc = err.user_descriptor();
    assert!(!desc.message().is_empty(), "error should have a safe message");
    assert_eq!(err.diagnostic_context().code(), "browser.read");
    assert!(!format!("{:?}", err).contains("private"), "debug output should not leak private data");
}
