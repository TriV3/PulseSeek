use std::path::Path;

use super::*;

fn probe_error_from(io_error: std::io::Error) -> ProbeError {
    ProbeError::from_io_error(io_error, Path::new("/test/a.wav"))
}

#[test]
fn probe_result_variants_are_distinct() {
    assert_ne!(ProbeResult::Directory, ProbeResult::Playable);
    assert_ne!(ProbeResult::Playable, ProbeResult::Unsupported);
    assert_ne!(ProbeResult::Unsupported, ProbeResult::Missing);
}

#[test]
fn probe_error_implements_error_contract() {
    let err = probe_error_from(std::io::Error::other("probe failed"));
    assert_eq!(err.user_descriptor().category(), ErrorCategory::Unavailable);
    let desc = err.user_descriptor();
    assert!(!desc.message().is_empty(), "error should have a safe message");
}

#[test]
fn probe_error_permission_denied_category() {
    let err = probe_error_from(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"));
    assert_eq!(err.user_descriptor().category(), ErrorCategory::PermissionDenied);
}

#[test]
fn probe_error_other_errors_are_unavailable() {
    let err = probe_error_from(std::io::Error::other("metadata failed"));
    assert_eq!(err.user_descriptor().category(), ErrorCategory::Unavailable);
}

#[test]
fn probe_error_display_describes_source() {
    let err = probe_error_from(std::io::Error::other("boom"));
    assert!(err.to_string().contains("path probe error"));
    assert!(err.source().is_some());
}

#[test]
fn probe_error_keeps_diagnostic_context() {
    let err = probe_error_from(std::io::Error::other("boom"));
    assert_eq!(err.diagnostic_context().code(), "file.operation");
}

#[test]
fn probe_error_category_returns_mapped_value() {
    let err = probe_error_from(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"));
    assert_eq!(err.category(), ErrorCategory::PermissionDenied);
}

/// A fake probe that records the received path so callers can assert the port
/// contract without touching the filesystem.
struct RecordingProbe;

impl ProbeFile for RecordingProbe {
    fn probe(&self, path: &Path) -> Result<ProbeResult, ProbeError> {
        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("");
        match name {
            "dir" => Ok(ProbeResult::Directory),
            "song.mp3" => Ok(ProbeResult::Playable),
            "notes.txt" => Ok(ProbeResult::Unsupported),
            "gone.mp3" => Ok(ProbeResult::Missing),
            _ => Err(probe_error_from(std::io::Error::other("unexpected"))),
        }
    }
}

#[test]
fn probe_port_reports_each_outcome() {
    let probe = RecordingProbe;
    assert_eq!(probe.probe(Path::new("/test/dir")).unwrap(), ProbeResult::Directory);
    assert_eq!(probe.probe(Path::new("/test/song.mp3")).unwrap(), ProbeResult::Playable);
    assert_eq!(probe.probe(Path::new("/test/notes.txt")).unwrap(), ProbeResult::Unsupported);
    assert_eq!(probe.probe(Path::new("/test/gone.mp3")).unwrap(), ProbeResult::Missing);
}

#[test]
fn probe_port_reports_inspection_failure() {
    let probe = RecordingProbe;
    let err = probe.probe(Path::new("/test/other")).unwrap_err();
    assert_eq!(err.category(), ErrorCategory::Unavailable);
}
