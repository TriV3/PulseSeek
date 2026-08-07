use super::*;
use pulseseek_domain::error::{DiagnosticCode, DiagnosticContext, ErrorCategory};

fn make_app_error(category: ErrorCategory) -> ApplicationError {
    ApplicationError::new(
        category,
        DiagnosticContext::new(DiagnosticCode::FileOperation),
        std::io::Error::other("fake error"),
    )
}

fn ok_service() -> FakeExternalService {
    FakeExternalService::new(Box::new(|_| Ok(())), Box::new(|_| Ok(())))
}

#[test]
fn external_service_reveal_success() {
    let service = ok_service();
    assert!(service.reveal("/music/a.wav".to_string()).is_ok());
}

#[test]
fn external_service_open_with_success() {
    let service = ok_service();
    assert!(service.open_with("/music/a.wav".to_string()).is_ok());
}

#[test]
fn external_service_reveal_missing_file_maps_to_not_found() {
    let service = FakeExternalService::new(
        Box::new(|_| Err(make_app_error(ErrorCategory::NotFound))),
        Box::new(|_| Ok(())),
    );
    let err = service.reveal("/music/missing.wav".to_string()).unwrap_err();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::NotFound);
}

#[test]
fn external_service_open_with_unsupported_platform_maps_to_unsupported() {
    let service = FakeExternalService::new(
        Box::new(|_| Ok(())),
        Box::new(|_| Err(make_app_error(ErrorCategory::Unsupported))),
    );
    let err = service.open_with("/music/a.wav".to_string()).unwrap_err();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::Unsupported);
}

#[test]
fn external_service_open_with_permission_denied_maps_to_permission_denied() {
    let service = FakeExternalService::new(
        Box::new(|_| Ok(())),
        Box::new(|_| Err(make_app_error(ErrorCategory::PermissionDenied))),
    );
    let err = service.open_with("/music/secret.wav".to_string()).unwrap_err();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::PermissionDenied);
}

#[test]
fn external_service_reveal_unavailable_maps_to_unavailable() {
    let service = FakeExternalService::new(
        Box::new(|_| Err(make_app_error(ErrorCategory::Unavailable))),
        Box::new(|_| Ok(())),
    );
    let err = service.reveal("/music/a.wav".to_string()).unwrap_err();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::Unavailable);
}
