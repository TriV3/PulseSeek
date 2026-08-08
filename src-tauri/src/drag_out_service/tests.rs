use super::*;
use pulseseek_domain::error::{DiagnosticCode, DiagnosticContext, ErrorCategory};

fn make_app_error(category: ErrorCategory) -> ApplicationError {
    ApplicationError::new(
        category,
        DiagnosticContext::new(DiagnosticCode::FileOperation),
        std::io::Error::other("fake error"),
    )
}

#[test]
fn drag_out_service_success() {
    let service = FakeDragOutService::new(Box::new(|_| Ok(())));
    assert!(service.drag_out(vec!["/music/a.wav".to_string()]).is_ok());
}

#[test]
fn drag_out_service_passes_all_paths() {
    let captured = std::sync::Arc::new(std::sync::Mutex::new(None::<Vec<String>>));
    let captured_for_closure = std::sync::Arc::clone(&captured);
    let service = FakeDragOutService::new(Box::new(move |paths| {
        *captured_for_closure.lock().unwrap() = Some(paths);
        Ok(())
    }));
    let paths = vec!["/music/a.wav".to_string(), "/music/b.wav".to_string()];
    assert!(service.drag_out(paths.clone()).is_ok());
    assert_eq!(*captured.lock().unwrap(), Some(paths));
}

#[test]
fn drag_out_service_missing_file_maps_to_not_found() {
    let service =
        FakeDragOutService::new(Box::new(|_| Err(make_app_error(ErrorCategory::NotFound))));
    let err = service.drag_out(vec!["/music/missing.wav".to_string()]).unwrap_err();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::NotFound);
}

#[test]
fn drag_out_service_cancellation_maps_to_cancelled() {
    let service =
        FakeDragOutService::new(Box::new(|_| Err(make_app_error(ErrorCategory::Cancelled))));
    let err = service.drag_out(vec!["/music/a.wav".to_string()]).unwrap_err();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::Cancelled);
}

#[test]
fn drag_out_service_unsupported_platform_maps_to_unsupported() {
    let service =
        FakeDragOutService::new(Box::new(|_| Err(make_app_error(ErrorCategory::Unsupported))));
    let err = service.drag_out(vec!["/music/a.wav".to_string()]).unwrap_err();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::Unsupported);
}

#[test]
fn drag_out_service_invalid_input_maps_to_invalid_input() {
    let service =
        FakeDragOutService::new(Box::new(|_| Err(make_app_error(ErrorCategory::InvalidInput))));
    let err = service.drag_out(vec![]).unwrap_err();
    assert_eq!(err.user_descriptor().category(), ErrorCategory::InvalidInput);
}
