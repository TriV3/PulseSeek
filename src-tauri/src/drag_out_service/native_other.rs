use std::path::PathBuf;

use pulseseek_browser_fs::drag_out::DragStarter;
use pulseseek_domain::browser::drag_out::DragOutError;

/// Fallback drag starter for platforms without a native drag-out adapter.
///
/// These platforms rely on the HTML5 `text/uri-list` drag initiated from the
/// webview, so the command path is reported as unsupported.
pub struct UnsupportedDragStarter;

impl DragStarter for UnsupportedDragStarter {
    fn start(&self, _paths: &[PathBuf]) -> Result<(), DragOutError> {
        Err(DragOutError::unsupported())
    }
}
