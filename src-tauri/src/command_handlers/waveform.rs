use std::path::PathBuf;
use std::sync::Arc;

use crate::command_envelope::{from_application_error, BoundaryError};
use crate::waveform_service::{WaveformLevel, WaveformRequest, WaveformService};

/// Handles the `get_waveform` command.
///
/// Maps a validated request to a waveform level, translating service errors
/// into boundary errors. The heavy extraction work runs on the waveform
/// worker via the service; this handler never touches an audio callback.
pub(crate) fn handle_get_waveform(
    path: &str,
    target_peaks: u64,
    service: &dyn WaveformService,
) -> Result<WaveformLevel, BoundaryError> {
    if path.trim().is_empty() {
        return Err(BoundaryError {
            category: "InvalidInput".to_string(),
            message: "Waveform path cannot be empty.".to_string(),
            diagnostic_code: "waveform.path".to_string(),
        });
    }
    let request = WaveformRequest { path: PathBuf::from(path), target_peaks };
    service.get_level(&request).map_err(|error| from_application_error(&error))
}

/// Tauri command: produces a waveform level for a file.
///
/// Extraction runs on a blocking worker through the waveform service, so a
/// long first render never blocks the webview main thread or the audio
/// callback. Playback is unaffected while waveform data is generated.
#[tauri::command]
pub async fn get_waveform(
    path: String,
    target_peaks: u64,
    service: tauri::State<'_, Arc<dyn WaveformService>>,
) -> Result<WaveformLevel, BoundaryError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        handle_get_waveform(&path, target_peaks, service.as_ref())
    })
    .await
    .map_err(|error| {
        tracing::error!(error = %error, "waveform worker failed");
        BoundaryError {
            category: "Internal".to_string(),
            message: "Waveform generation worker failed.".to_string(),
            diagnostic_code: "waveform.worker".to_string(),
        }
    })?
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use pulseseek_domain::error::{
        ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory,
    };

    use crate::command_envelope::{from_application_error, BoundaryError};
    use crate::waveform_service::{WaveformLevel, WaveformRequest, WaveformService};

    use super::handle_get_waveform;

    enum FakeOutcome {
        Level(WaveformLevel),
        Unavailable,
    }

    struct FakeService {
        outcome: Mutex<FakeOutcome>,
        requests: Mutex<Vec<WaveformRequest>>,
    }

    impl FakeService {
        fn ok() -> Self {
            Self {
                outcome: Mutex::new(FakeOutcome::Level(sample_level())),
                requests: Mutex::new(Vec::new()),
            }
        }

        fn unavailable() -> Self {
            Self { outcome: Mutex::new(FakeOutcome::Unavailable), requests: Mutex::new(Vec::new()) }
        }
    }

    fn sample_level() -> WaveformLevel {
        WaveformLevel {
            format_version: 1,
            channels: 2,
            samples_per_peak: 4,
            min: vec![-0.5, -0.25],
            max: vec![0.5, 0.75],
        }
    }

    impl WaveformService for FakeService {
        fn get_level(&self, request: &WaveformRequest) -> Result<WaveformLevel, ApplicationError> {
            self.requests.lock().unwrap().push(request.clone());
            match &*self.outcome.lock().unwrap() {
                FakeOutcome::Level(level) => Ok(level.clone()),
                FakeOutcome::Unavailable => Err(ApplicationError::new(
                    ErrorCategory::Unavailable,
                    DiagnosticContext::new(DiagnosticCode::BrowserRead),
                    std::io::Error::other("fake failure"),
                )),
            }
        }
    }

    #[test]
    fn valid_request_returns_level() {
        let service = FakeService::ok();
        let level =
            handle_get_waveform("/music/track.wav", 4, &service).expect("valid request succeeds");

        assert_eq!(level.channels, 2);
        assert_eq!(level.samples_per_peak, 4);
        assert_eq!(level.min, vec![-0.5, -0.25]);
        assert_eq!(level.max, vec![0.5, 0.75]);

        let requests = service.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].path, PathBuf::from("/music/track.wav"));
        assert_eq!(requests[0].target_peaks, 4);
    }

    #[test]
    fn empty_path_is_rejected() {
        let service = FakeService::ok();
        let error = handle_get_waveform("  ", 4, &service).expect_err("empty path rejected");

        assert_eq!(error.category, "InvalidInput");
        assert_eq!(error.diagnostic_code, "waveform.path");
        assert!(service.requests.lock().unwrap().is_empty(), "service must not run");
    }

    #[test]
    fn service_error_maps_to_boundary() {
        let service = FakeService::unavailable();
        let error =
            handle_get_waveform("/music/track.wav", 4, &service).expect_err("service error");

        assert_eq!(error.category, "Unavailable");
        assert_eq!(error.diagnostic_code, "browser.read");
    }

    #[test]
    fn boundary_error_round_trip_matches_envelope() {
        let error = from_application_error(&ApplicationError::new(
            ErrorCategory::Unavailable,
            DiagnosticContext::new(DiagnosticCode::BrowserRead),
            std::io::Error::other("fake failure"),
        ));
        assert!(matches!(error, BoundaryError { .. }));
    }
}
