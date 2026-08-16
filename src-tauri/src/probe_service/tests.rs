use std::path::Path;

use pulseseek_domain::browser::probe::{ProbeError, ProbeFile, ProbeResult};
use pulseseek_domain::error::{DiagnosticCode, DiagnosticContext, ErrorCategory};

use super::*;

struct StubProbe {
    result: Option<ProbeResult>,
    permission_denied: bool,
}

impl ProbeFile for StubProbe {
    fn probe(&self, _path: &Path) -> Result<ProbeResult, ProbeError> {
        if self.permission_denied {
            return Err(ProbeError::from_io_error(
                std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
                Path::new("/music/a.wav"),
            ));
        }
        Ok(self.result.expect("configure a result"))
    }
}

fn probe_kind_adapter(result: ProbeResult) -> GenericNativeProbeService<StubProbe> {
    GenericNativeProbeService::new(StubProbe { result: Some(result), permission_denied: false })
}

fn probe_error_adapter() -> GenericNativeProbeService<StubProbe> {
    GenericNativeProbeService::new(StubProbe { result: None, permission_denied: true })
}

fn make_app_error(category: ErrorCategory) -> ApplicationError {
    ApplicationError::new(
        category,
        DiagnosticContext::new(DiagnosticCode::FileOperation),
        std::io::Error::other("fake error"),
    )
}

#[test]
fn probe_kind_maps_directory() {
    assert_eq!(ProbeKind::from(ProbeResult::Directory), ProbeKind::Directory);
}

#[test]
fn probe_kind_maps_playable() {
    assert_eq!(ProbeKind::from(ProbeResult::Playable), ProbeKind::Playable);
}

#[test]
fn probe_kind_maps_unsupported() {
    assert_eq!(ProbeKind::from(ProbeResult::Unsupported), ProbeKind::Unsupported);
}

#[test]
fn probe_kind_maps_missing() {
    assert_eq!(ProbeKind::from(ProbeResult::Missing), ProbeKind::Missing);
}

#[test]
fn probe_service_passes_path_and_kind() {
    let service = probe_kind_adapter(ProbeResult::Playable);
    assert_eq!(service.probe("/music/a.wav".to_string()).unwrap(), ProbeKind::Playable);
}

#[test]
fn probe_service_maps_directory() {
    let service = probe_kind_adapter(ProbeResult::Directory);
    assert_eq!(service.probe("/music/folder".to_string()).unwrap(), ProbeKind::Directory);
}

#[test]
fn probe_service_maps_unsupported() {
    let service = probe_kind_adapter(ProbeResult::Unsupported);
    assert_eq!(service.probe("/music/notes.txt".to_string()).unwrap(), ProbeKind::Unsupported);
}

#[test]
fn probe_service_maps_missing() {
    let service = probe_kind_adapter(ProbeResult::Missing);
    assert_eq!(service.probe("/music/gone.wav".to_string()).unwrap(), ProbeKind::Missing);
}

#[test]
fn probe_service_maps_permission_error() {
    let service = probe_error_adapter();
    let err = service.probe("/music/a.wav".to_string()).unwrap_err();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::PermissionDenied);
}

#[test]
fn fake_probe_service_reports_kind() {
    let service = FakeProbeService::new(Box::new(|_| Ok(ProbeKind::Playable)));
    assert_eq!(service.probe("/music/a.wav".to_string()).unwrap(), ProbeKind::Playable);
}

#[test]
fn fake_probe_service_reports_error() {
    let service =
        FakeProbeService::new(Box::new(|_| Err(make_app_error(ErrorCategory::Unavailable))));
    let err = service.probe("/music/a.wav".to_string()).unwrap_err();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::Unavailable);
}
