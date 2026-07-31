use std::path::PathBuf;

use super::*;
use pulseseek_domain::error::{DiagnosticCode, DiagnosticContext, ErrorCategory, ErrorContract};

fn make_app_error(category: ErrorCategory) -> ApplicationError {
    ApplicationError::new(
        category,
        DiagnosticContext::new(DiagnosticCode::FileOperation),
        std::io::Error::other("fake error"),
    )
}

#[test]
fn trash_service_returns_one_result_per_path() {
    let paths = vec![PathBuf::from("/music/a.wav"), PathBuf::from("/music/b.wav")];
    let service = FakeTrashService::new(Box::new(|_| {
        vec![(PathBuf::from("/music/a.wav"), Ok(())), (PathBuf::from("/music/b.wav"), Ok(()))]
    }));
    let output = service.move_to_trash(paths);
    assert_eq!(output.len(), 2);
    assert!(output[0].1.is_ok());
    assert!(output[1].1.is_ok());
}

#[test]
fn trash_service_reports_permission_failure() {
    let paths = vec![PathBuf::from("/music/secret.wav")];
    let service = FakeTrashService::new(Box::new(|_| {
        let err = make_app_error(ErrorCategory::PermissionDenied);
        vec![(PathBuf::from("/music/secret.wav"), Err(err))]
    }));
    let output = service.move_to_trash(paths);
    assert_eq!(output.len(), 1);
    assert!(output[0].1.is_err());
    assert_eq!(
        output[0].1.as_ref().unwrap_err().user_descriptor().category(),
        ErrorCategory::PermissionDenied
    );
}

#[test]
fn trash_service_reports_cancellation() {
    let paths = vec![PathBuf::from("/music/a.wav")];
    let service = FakeTrashService::new(Box::new(|_| {
        let err = make_app_error(ErrorCategory::Cancelled);
        vec![(PathBuf::from("/music/a.wav"), Err(err))]
    }));
    let output = service.move_to_trash(paths);
    assert_eq!(output.len(), 1);
    assert!(output[0].1.is_err());
    assert_eq!(
        output[0].1.as_ref().unwrap_err().user_descriptor().category(),
        ErrorCategory::Cancelled
    );
}

#[test]
fn trash_service_reports_partial_batch_failure() {
    let paths = vec![PathBuf::from("/music/good.wav"), PathBuf::from("/music/bad.wav")];
    let service = FakeTrashService::new(Box::new(|_| {
        let err = make_app_error(ErrorCategory::NotFound);
        vec![
            (PathBuf::from("/music/good.wav"), Ok(())),
            (PathBuf::from("/music/bad.wav"), Err(err)),
        ]
    }));
    let output = service.move_to_trash(paths);
    assert_eq!(output.len(), 2);
    assert!(output[0].1.is_ok());
    assert!(output[1].1.is_err());
    assert_eq!(
        output[1].1.as_ref().unwrap_err().user_descriptor().category(),
        ErrorCategory::NotFound
    );
}

#[test]
fn trash_service_accepts_empty_list() {
    let service = FakeTrashService::new(Box::new(|_| vec![]));
    let output = service.move_to_trash(vec![]);
    assert!(output.is_empty());
}
