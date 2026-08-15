use std::path::PathBuf;
use std::sync::Mutex;

use tauri::Url;

/// Pending audio files the operating system asked PulseSeek to open.
///
/// macOS delivers file opens in two ways: as command-line arguments on a cold
/// launch (double-clicking a document in Finder) and through
/// [`tauri::RunEvent::Opened`] while the app is running. Both paths feed this
/// state; the frontend drains it once ready so no open is lost when it happens
/// before the webview subscribes.
#[derive(Default)]
pub struct OpenedFiles(Mutex<Vec<String>>);

impl OpenedFiles {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds paths that are not already pending. Duplicates from argv and the
    /// Apple event route are collapsed so a file is only opened once.
    pub fn collect(&self, paths: Vec<String>) {
        let mut pending = self.0.lock().expect("opened files mutex poisoned");
        for path in paths {
            if !pending.contains(&path) {
                pending.push(path);
            }
        }
    }

    /// Returns and clears every pending path.
    pub fn take(&self) -> Vec<String> {
        let mut pending = self.0.lock().expect("opened files mutex poisoned");
        std::mem::take(&mut *pending)
    }
}

/// Converts a `file://` URL into a filesystem path, rejecting non-file URLs.
pub fn file_url_to_path(url: &Url) -> Option<PathBuf> {
    url.to_file_path().ok()
}

/// Returns the pending opened files and clears the queue (cold-start path).
#[tauri::command]
pub fn opened_audio_files(state: tauri::State<'_, OpenedFiles>) -> Vec<String> {
    state.take()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn collect_adds_paths() {
        let opened = OpenedFiles::new();
        opened.collect(vec!["/music/a.wav".to_string(), "/music/b.mp3".to_string()]);
        assert_eq!(opened.take(), vec!["/music/a.wav", "/music/b.mp3"]);
    }

    #[test]
    fn collect_dedupes_existing_paths() {
        let opened = OpenedFiles::new();
        opened.collect(vec!["/music/a.wav".to_string()]);
        opened.collect(vec!["/music/a.wav".to_string(), "/music/b.mp3".to_string()]);
        assert_eq!(opened.take(), vec!["/music/a.wav", "/music/b.mp3"]);
    }

    #[test]
    fn take_drains_the_queue() {
        let opened = OpenedFiles::new();
        opened.collect(vec!["/music/a.wav".to_string()]);
        assert_eq!(opened.take().len(), 1);
        assert!(opened.take().is_empty(), "second take must be empty");
    }

    #[test]
    fn file_url_converts_to_path() {
        let url = Url::from_str("file:///Users/test/Music/song.wav").unwrap();
        let path = file_url_to_path(&url).expect("file URL converts");
        assert_eq!(path, PathBuf::from("/Users/test/Music/song.wav"));
    }

    #[test]
    fn non_file_url_is_rejected() {
        let url = Url::from_str("https://example.com/song.wav").unwrap();
        assert!(file_url_to_path(&url).is_none());
    }
}
