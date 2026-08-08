use std::path::PathBuf;

use pulseseek_domain::error::{DiagnosticCode, DiagnosticContext, ErrorCategory};
use pulseseek_domain::playback::mode::PlaybackMode;

use super::*;
use crate::audio_device_service::{DeviceInfoData, FakeAudioDeviceService};
use crate::copy_service::{CopyService, FakeCopyService};
use crate::dialog_service::FakeFolderPicker;
use crate::drag_out_service::{DragOutService, FakeDragOutService};
use crate::external_service::{ExternalService, FakeExternalService};
use crate::folder_enumeration_service::{ActiveEnumerations, FakeFolderEnumerationService};
use crate::move_service::{FakeMoveService, MoveService};
use crate::playback_events::{FakeEventEmitter, NoopEventEmitter};
use crate::playback_service::FakePlaybackService;
use crate::recent_folders_service::InMemoryRecentFoldersService;
use crate::rename_service::{FakeRenameService, RenameOutcome};
use crate::trash_service::FakeTrashService;

fn noop_events() -> Arc<dyn PlaybackEventEmitter> {
    Arc::new(NoopEventEmitter)
}

fn fake_events() -> (Arc<FakeEventEmitter>, Arc<dyn PlaybackEventEmitter>) {
    let inner = Arc::new(FakeEventEmitter::new());
    let erased = inner.clone() as Arc<dyn PlaybackEventEmitter>;
    (inner, erased)
}

fn noop_device() -> FakeAudioDeviceService {
    FakeAudioDeviceService::new()
}

fn noop_enum() -> FakeFolderEnumerationService {
    FakeFolderEnumerationService::new()
}

fn noop_active() -> ActiveEnumerations {
    ActiveEnumerations::new()
}

fn noop_trash() -> FakeTrashService {
    FakeTrashService::new(Box::new(|_| vec![]))
}

fn noop_rename() -> FakeRenameService {
    FakeRenameService::new(Box::new(|_, _| {
        Err(ApplicationError::new(
            ErrorCategory::Unavailable,
            DiagnosticContext::new(DiagnosticCode::FileOperation),
            std::io::Error::other("fake rename not configured"),
        ))
    }))
}

fn noop_move() -> FakeMoveService {
    FakeMoveService::new(Box::new(|_, _, _| {
        Err(ApplicationError::new(
            ErrorCategory::Unavailable,
            DiagnosticContext::new(DiagnosticCode::FileOperation),
            std::io::Error::other("fake move not configured"),
        ))
    }))
}

fn noop_copy() -> FakeCopyService {
    FakeCopyService::new(Box::new(|_, _, _| {
        Err(ApplicationError::new(
            ErrorCategory::Unavailable,
            DiagnosticContext::new(DiagnosticCode::FileOperation),
            std::io::Error::other("fake copy not configured"),
        ))
    }))
}

fn noop_external() -> FakeExternalService {
    FakeExternalService::new(
        Box::new(|_| {
            Err(ApplicationError::new(
                ErrorCategory::Unavailable,
                DiagnosticContext::new(DiagnosticCode::FileOperation),
                std::io::Error::other("fake external not configured"),
            ))
        }),
        Box::new(|_| {
            Err(ApplicationError::new(
                ErrorCategory::Unavailable,
                DiagnosticContext::new(DiagnosticCode::FileOperation),
                std::io::Error::other("fake external not configured"),
            ))
        }),
    )
}

fn noop_drag_out() -> FakeDragOutService {
    FakeDragOutService::new(Box::new(|_| {
        Err(ApplicationError::new(
            ErrorCategory::Unavailable,
            DiagnosticContext::new(DiagnosticCode::FileOperation),
            std::io::Error::other("fake drag-out not configured"),
        ))
    }))
}

fn noop_recent() -> InMemoryRecentFoldersService {
    InMemoryRecentFoldersService::new()
}

fn health_envelope() -> CommandEnvelope {
    CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "health".to_string(),
        payload: serde_json::json!({}),
    }
}

// ── Health command tests ──────────────────────────────────────────

#[test]
fn health_command_round_trip() {
    let mut service = FakePlaybackService::new();

    let response = dispatch(
        health_envelope(),
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(response.ok);
    let data = response.data.expect("should have data");
    assert_eq!(data, serde_json::json!({"ready": true}));
    assert!(response.error.is_none());
}

#[test]
fn unknown_version_rejected() {
    let mut service = FakePlaybackService::new();

    for bad_version in [0, 99] {
        let envelope = CommandEnvelope {
            version: bad_version,
            command: "health".to_string(),
            payload: serde_json::json!({}),
        };

        let response = dispatch(
            envelope,
            &mut service,
            &mut noop_device(),
            &mut noop_enum(),
            &noop_trash(),
            &noop_rename(),
            &noop_move(),
            &noop_copy(),
            &noop_external(),
            &noop_drag_out(),
            &noop_active(),
            &noop_recent(),
            &noop_events(),
        );

        assert!(!response.ok);
        let error = response.error.expect("should have error");
        assert_eq!(error.category, "Unsupported");
        assert!(error.message.contains(&bad_version.to_string()));
        assert_eq!(error.diagnostic_code, "command.version");
    }
}

#[test]
fn unknown_command_rejected() {
    let mut service = FakePlaybackService::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "nonexistent".to_string(),
        payload: serde_json::json!({}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    let error = response.error.expect("should have error");
    assert_eq!(error.category, "Unsupported");
    assert!(error.message.contains("nonexistent"));
    assert_eq!(error.diagnostic_code, "command.unknown");
}

#[test]
fn invalid_payload_rejected() {
    let mut service = FakePlaybackService::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "health".to_string(),
        payload: serde_json::json!("not_an_object"),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    let error = response.error.expect("should have error");
    assert_eq!(error.category, "InvalidInput");
    assert_eq!(error.diagnostic_code, "command.payload");
}

#[test]
fn application_error_maps_to_boundary_error() {
    let adapter_error =
        std::io::Error::new(std::io::ErrorKind::PermissionDenied, "/Users/alice/secret/file.wav");
    let app_error = ApplicationError::new(
        ErrorCategory::PermissionDenied,
        DiagnosticContext::new(DiagnosticCode::BrowserRead),
        adapter_error,
    );

    let boundary = from_application_error(&app_error);

    assert_eq!(boundary.category, "PermissionDenied");
    assert_eq!(boundary.message, "PulseSeek could not access that item.");
    assert_eq!(boundary.diagnostic_code, "browser.read");
    assert!(!boundary.message.contains("alice"));
    assert!(!boundary.message.contains("secret"));
}

// ── Play command tests ────────────────────────────────────────────

#[test]
fn play_command_dispatches_to_service() {
    let mut service = FakePlaybackService::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "play".to_string(),
        payload: serde_json::json!({"path": "/music/track.wav"}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(response.ok, "play should succeed");
    assert!(response.error.is_none());
    assert_eq!(service.play_call_count, 1);
    assert_eq!(service.last_play_path, Some("/music/track.wav".to_string()));
}

#[test]
fn play_command_invalid_payload_rejected() {
    let mut service = FakePlaybackService::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "play".to_string(),
        payload: serde_json::json!("not_an_object"),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    let error = response.error.expect("should have error");
    assert_eq!(error.category, "InvalidInput");
    assert_eq!(error.diagnostic_code, "command.payload");
    assert_eq!(service.play_call_count, 0, "service should not be called");
}

#[test]
fn play_command_missing_path_rejected() {
    let mut service = FakePlaybackService::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "play".to_string(),
        payload: serde_json::json!({}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    let error = response.error.expect("should have error");
    assert_eq!(error.category, "InvalidInput");
    assert_eq!(service.play_call_count, 0, "service should not be called");
}

#[test]
fn set_playback_mode_dispatches_and_returns_confirmed_mode() {
    let mut service = FakePlaybackService::new();
    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "set_playback_mode".to_string(),
        payload: serde_json::json!({"mode": "loop-current"}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(response.ok);
    assert_eq!(service.mode, PlaybackMode::LoopCurrent);
    assert_eq!(response.data.unwrap()["mode"], "loop-current");
}

#[test]
fn set_playback_mode_rejects_unknown_mode() {
    let mut service = FakePlaybackService::new();
    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "set_playback_mode".to_string(),
        payload: serde_json::json!({"mode": "invalid"}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    assert_eq!(response.error.unwrap().diagnostic_code, "command.mode");
    assert_eq!(service.mode, PlaybackMode::OneShot);
}

#[test]
fn set_playback_mode_maps_service_failure() {
    let mut service = FakePlaybackService::new();
    service.fail_with = Some(ErrorCategory::Unavailable);
    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "set_playback_mode".to_string(),
        payload: serde_json::json!({"mode": "random"}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    assert_eq!(response.error.unwrap().category, "Unavailable");
    assert_eq!(service.mode, PlaybackMode::OneShot);
}

// ── Pause command tests ───────────────────────────────────────────

#[test]
fn pause_command_dispatches_to_service() {
    let mut service = FakePlaybackService::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "pause".to_string(),
        payload: serde_json::json!({}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(response.ok);
    assert_eq!(service.pause_call_count, 1);
}

#[test]
fn pause_command_invalid_payload_rejected() {
    let mut service = FakePlaybackService::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "pause".to_string(),
        payload: serde_json::json!("invalid"),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    assert_eq!(response.error.unwrap().category, "InvalidInput");
    assert_eq!(service.pause_call_count, 0);
}

// ── Resume command tests ──────────────────────────────────────────

#[test]
fn resume_command_dispatches_to_service() {
    let mut service = FakePlaybackService::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "resume".to_string(),
        payload: serde_json::json!({}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(response.ok);
    assert_eq!(service.resume_call_count, 1);
}

#[test]
fn resume_command_invalid_payload_rejected() {
    let mut service = FakePlaybackService::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "resume".to_string(),
        payload: serde_json::json!("invalid"),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    assert_eq!(service.resume_call_count, 0);
}

// ── Stop command tests ────────────────────────────────────────────

#[test]
fn stop_command_dispatches_to_service() {
    let mut service = FakePlaybackService::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "stop".to_string(),
        payload: serde_json::json!({}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(response.ok);
    assert_eq!(service.stop_call_count, 1);
}

#[test]
fn stop_command_invalid_payload_rejected() {
    let mut service = FakePlaybackService::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "stop".to_string(),
        payload: serde_json::json!("invalid"),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    assert_eq!(service.stop_call_count, 0);
}

// ── Seek command tests ────────────────────────────────────────────

#[test]
fn seek_command_dispatches_to_service() {
    let mut service = FakePlaybackService::new();
    service.seek_result = Some(45000);

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "seek".to_string(),
        payload: serde_json::json!({"position_ms": 45000}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(response.ok);
    assert_eq!(service.seek_call_count, 1);
    assert_eq!(service.last_seek_position, Some(45000));
    let data = response.data.expect("should have data");
    assert_eq!(data, serde_json::json!({"position_ms": 45000}));
}

#[test]
fn seek_command_invalid_payload_rejected() {
    let mut service = FakePlaybackService::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "seek".to_string(),
        payload: serde_json::json!("not_an_object"),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    assert_eq!(service.seek_call_count, 0);
}

#[test]
fn seek_command_missing_position_rejected() {
    let mut service = FakePlaybackService::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "seek".to_string(),
        payload: serde_json::json!({}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    assert_eq!(service.seek_call_count, 0);
}

// ── Volume command tests ──────────────────────────────────────────

#[test]
fn volume_command_dispatches_to_service() {
    let mut service = FakePlaybackService::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "volume".to_string(),
        payload: serde_json::json!({"gain": 0.75, "muted": false}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(response.ok);
    assert_eq!(service.set_volume_call_count, 1);
    assert!((service.last_volume_gain.unwrap() - 0.75).abs() < f64::EPSILON);
    assert_eq!(service.last_volume_muted, Some(false));
}

#[test]
fn volume_command_muted_state_dispatches_to_service() {
    let mut service = FakePlaybackService::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "volume".to_string(),
        payload: serde_json::json!({"gain": 0.0, "muted": true}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(response.ok);
    assert_eq!(service.set_volume_call_count, 1);
    assert_eq!(service.last_volume_muted, Some(true));
}

#[test]
fn volume_command_invalid_payload_rejected() {
    let mut service = FakePlaybackService::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "volume".to_string(),
        payload: serde_json::json!("not_an_object"),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    assert_eq!(service.set_volume_call_count, 0);
}

#[test]
fn volume_command_missing_fields_rejected() {
    let mut service = FakePlaybackService::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "volume".to_string(),
        payload: serde_json::json!({}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    assert_eq!(service.set_volume_call_count, 0);
}

// ── Service error propagation tests ───────────────────────────────

#[test]
fn play_service_error_maps_to_boundary_error() {
    let mut service = FakePlaybackService::new();
    service.fail_with = Some(ErrorCategory::Unavailable);

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "play".to_string(),
        payload: serde_json::json!({"path": "/music/track.wav"}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    let error = response.error.expect("should have error");
    assert_eq!(error.category, "Unavailable");
    assert_eq!(error.diagnostic_code, "playback.control");
}

#[test]
fn pause_service_error_maps_to_boundary_error() {
    let mut service = FakePlaybackService::new();
    service.fail_with = Some(ErrorCategory::Conflict);

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "pause".to_string(),
        payload: serde_json::json!({}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    let error = response.error.expect("should have error");
    assert_eq!(error.category, "Conflict");
}

#[test]
fn resume_service_error_maps_to_boundary_error() {
    let mut service = FakePlaybackService::new();
    service.fail_with = Some(ErrorCategory::Unavailable);

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "resume".to_string(),
        payload: serde_json::json!({}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    let error = response.error.expect("should have error");
    assert_eq!(error.category, "Unavailable");
}

#[test]
fn stop_service_error_maps_to_boundary_error() {
    let mut service = FakePlaybackService::new();
    service.fail_with = Some(ErrorCategory::NotFound);

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "stop".to_string(),
        payload: serde_json::json!({}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    let error = response.error.expect("should have error");
    assert_eq!(error.category, "NotFound");
}

#[test]
fn seek_service_error_maps_to_boundary_error() {
    let mut service = FakePlaybackService::new();
    service.fail_with = Some(ErrorCategory::InvalidInput);

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "seek".to_string(),
        payload: serde_json::json!({"position_ms": 99999}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    let error = response.error.expect("should have error");
    assert_eq!(error.category, "InvalidInput");
}

#[test]
fn volume_service_error_maps_to_boundary_error() {
    let mut service = FakePlaybackService::new();
    service.fail_with = Some(ErrorCategory::Unsupported);

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "volume".to_string(),
        payload: serde_json::json!({"gain": 1.0, "muted": false}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    let error = response.error.expect("should have error");
    assert_eq!(error.category, "Unsupported");
}

// ── Event emission tests ─────────────────────────────────────────

#[test]
fn play_emits_playing_state_event() {
    let mut service = FakePlaybackService::new();
    let (events_inner, events) = fake_events();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "play".to_string(),
        payload: serde_json::json!({"path": "/music/track.wav"}),
    };

    let _response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &events,
    );

    let recorded = events_inner.recorded_events();
    assert_eq!(recorded.len(), 1, "play should emit one event");
    assert_eq!(recorded[0].event, "playback:state-changed");
    let payload: serde_json::Value = serde_json::from_value(recorded[0].payload.clone()).unwrap();
    assert_eq!(payload["state"], "playing");
}

#[test]
fn play_emits_event_only_on_success() {
    let mut service = FakePlaybackService::new();
    service.fail_with = Some(ErrorCategory::NotFound);
    let (events_inner, events) = fake_events();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "play".to_string(),
        payload: serde_json::json!({"path": "/music/missing.wav"}),
    };

    let _response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &events,
    );

    assert_eq!(events_inner.event_count(), 0, "no event on error");
}

#[test]
fn pause_emits_paused_state_event() {
    let mut service = FakePlaybackService::new();
    let (events_inner, events) = fake_events();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "pause".to_string(),
        payload: serde_json::json!({}),
    };

    let _response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &events,
    );

    let recorded = events_inner.recorded_events();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].event, "playback:state-changed");
    let payload: serde_json::Value = serde_json::from_value(recorded[0].payload.clone()).unwrap();
    assert_eq!(payload["state"], "paused");
}

#[test]
fn resume_emits_playing_state_event() {
    let mut service = FakePlaybackService::new();
    let (events_inner, events) = fake_events();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "resume".to_string(),
        payload: serde_json::json!({}),
    };

    let _response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &events,
    );

    let recorded = events_inner.recorded_events();
    assert_eq!(recorded.len(), 1);
    let payload: serde_json::Value = serde_json::from_value(recorded[0].payload.clone()).unwrap();
    assert_eq!(payload["state"], "playing");
}

#[test]
fn stop_emits_stopped_state_event() {
    let mut service = FakePlaybackService::new();
    let (events_inner, events) = fake_events();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "stop".to_string(),
        payload: serde_json::json!({}),
    };

    let _response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &events,
    );

    let recorded = events_inner.recorded_events();
    assert_eq!(recorded.len(), 1);
    let payload: serde_json::Value = serde_json::from_value(recorded[0].payload.clone()).unwrap();
    assert_eq!(payload["state"], "stopped");
}

#[test]
fn seek_does_not_emit_state_event() {
    let mut service = FakePlaybackService::new();
    let (events_inner, events) = fake_events();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "seek".to_string(),
        payload: serde_json::json!({"position_ms": 45000}),
    };

    let _response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &events,
    );

    assert_eq!(events_inner.event_count(), 0, "seek should not emit state event");
}

#[test]
fn volume_does_not_emit_state_event() {
    let mut service = FakePlaybackService::new();
    let (events_inner, events) = fake_events();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "volume".to_string(),
        payload: serde_json::json!({"gain": 0.5, "muted": false}),
    };

    let _response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &events,
    );

    assert_eq!(events_inner.event_count(), 0, "volume should not emit state event");
}

#[test]
fn state_event_has_versioned_envelope() {
    let mut service = FakePlaybackService::new();
    let (events_inner, events) = fake_events();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "play".to_string(),
        payload: serde_json::json!({"path": "/music/track.wav"}),
    };

    let _response = dispatch(
        envelope,
        &mut service,
        &mut noop_device(),
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &events,
    );

    let recorded = events_inner.recorded_events();
    assert_eq!(recorded[0].version, crate::playback_events::CURRENT_EVENT_VERSION);
}

// ── Device command tests ─────────────────────────────────────────

#[test]
fn list_devices_command_returns_devices() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "list_devices".to_string(),
        payload: serde_json::json!({}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(response.ok);
    let data = response.data.expect("should have data");
    let devices: Vec<DeviceInfoData> = serde_json::from_value(data["devices"].clone()).unwrap();
    assert_eq!(devices.len(), 2);
    assert_eq!(devices[0].id, "default");
    assert_eq!(devices[0].name, "Default Output");
}

#[test]
fn list_devices_command_invalid_payload_rejected() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "list_devices".to_string(),
        payload: serde_json::json!("not_an_object"),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    assert_eq!(response.error.unwrap().category, "InvalidInput");
}

#[test]
fn list_devices_service_error_maps_to_boundary() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();
    device_service.fail_list = true;

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "list_devices".to_string(),
        payload: serde_json::json!({}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    let error = response.error.unwrap();
    assert_eq!(error.category, "Unavailable");
    assert_eq!(error.diagnostic_code, "audio.output");
}

#[test]
fn current_device_command_returns_none_when_no_device() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();
    device_service.current = None;

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "current_device".to_string(),
        payload: serde_json::json!({}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(response.ok);
    let data = response.data.expect("should have data");
    assert!(data["device"].is_null());
}

#[test]
fn current_device_command_returns_device_when_set() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "current_device".to_string(),
        payload: serde_json::json!({}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(response.ok);
    let data = response.data.expect("should have data");
    assert_eq!(data["device"]["id"], "default");
    assert_eq!(data["device"]["name"], "Default Output");
}

#[test]
fn current_device_service_error_maps_to_boundary() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();
    device_service.fail_current = true;

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "current_device".to_string(),
        payload: serde_json::json!({}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    assert_eq!(response.error.unwrap().category, "Unavailable");
}

#[test]
fn select_device_command_dispatches_to_service() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "select_device".to_string(),
        payload: serde_json::json!({"device_id": "hdmi"}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(response.ok);
    assert_eq!(service.select_output_device_call_count, 1);
    assert_eq!(service.last_output_device_id, Some("hdmi".to_string()));
    assert_eq!(device_service.select_call_count, 0);
}

#[test]
fn select_device_command_invalid_payload_rejected() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "select_device".to_string(),
        payload: serde_json::json!("not_an_object"),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    assert_eq!(device_service.select_call_count, 0);
    assert_eq!(service.select_output_device_call_count, 0);
}

#[test]
fn select_device_command_missing_id_rejected() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "select_device".to_string(),
        payload: serde_json::json!({}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    assert_eq!(device_service.select_call_count, 0);
    assert_eq!(service.select_output_device_call_count, 0);
}

#[test]
fn select_device_service_error_maps_to_boundary() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();
    service.fail_with = Some(ErrorCategory::Unavailable);

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "select_device".to_string(),
        payload: serde_json::json!({"device_id": "hdmi"}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    assert_eq!(response.error.unwrap().category, "Unavailable");
}

#[test]
fn select_device_emits_device_lost_event() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();
    device_service.device_lost = true;
    let (events_inner, events) = fake_events();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "select_device".to_string(),
        payload: serde_json::json!({"device_id": "hdmi"}),
    };

    let _response = dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &events,
    );

    let recorded = events_inner.recorded_events();
    assert_eq!(recorded.len(), 1, "should emit device-lost event");
    assert_eq!(recorded[0].event, "audio:device-lost");
    let payload: serde_json::Value = serde_json::from_value(recorded[0].payload.clone()).unwrap();
    assert_eq!(payload["previous_device_id"], "hdmi");
}

#[test]
fn select_device_does_not_emit_device_lost_when_not_lost() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();
    device_service.device_lost = false;
    let (events_inner, events) = fake_events();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "select_device".to_string(),
        payload: serde_json::json!({"device_id": "hdmi"}),
    };

    let _response = dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &events,
    );

    assert_eq!(events_inner.event_count(), 0, "should not emit event when device not lost");
}

#[test]
fn unknown_device_command_rejected() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "unknown_device_op".to_string(),
        payload: serde_json::json!({}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut noop_enum(),
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &noop_active(),
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    assert_eq!(response.error.unwrap().category, "Unsupported");
}

// ── Enumeration command tests ─────────────────────────────────────

#[test]
fn start_enumeration_returns_session_id() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();
    let mut enum_service = FakeFolderEnumerationService::new();
    let active = ActiveEnumerations::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "start_enumeration".to_string(),
        payload: serde_json::json!({"path": "/music", "batch_size": 50}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut enum_service,
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &active,
        &noop_recent(),
        &noop_events(),
    );

    assert!(response.ok);
    assert_eq!(enum_service.start_call_count, 1);
    assert_eq!(enum_service.last_path, Some("/music".to_string()));
    let data = response.data.expect("should have data");
    assert_eq!(data["session_id"], "test-session-001");
}

#[test]
fn list_browser_roots_returns_typed_roots() {
    let mut playback = FakePlaybackService::new();
    let mut devices = FakeAudioDeviceService::new();
    let mut enumeration = FakeFolderEnumerationService::new();
    enumeration.roots = vec![
        crate::folder_enumeration_service::BrowserRootData {
            path: "/".to_string(),
            name: "System".to_string(),
        },
        crate::folder_enumeration_service::BrowserRootData {
            path: "/Volumes/NAS".to_string(),
            name: "NAS".to_string(),
        },
    ];
    let active = ActiveEnumerations::new();

    let response = dispatch(
        CommandEnvelope {
            version: CURRENT_COMMAND_VERSION,
            command: "list_browser_roots".to_string(),
            payload: serde_json::json!({}),
        },
        &mut playback,
        &mut devices,
        &mut enumeration,
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &active,
        &noop_recent(),
        &noop_events(),
    );

    assert!(response.ok);
    let roots = response.data.unwrap()["roots"].as_array().unwrap().clone();
    assert_eq!(roots.len(), 2);
    assert_eq!(roots[1]["name"], "NAS");
}

#[test]
fn start_enumeration_defaults_batch_size() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();
    let mut enum_service = FakeFolderEnumerationService::new();
    let active = ActiveEnumerations::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "start_enumeration".to_string(),
        payload: serde_json::json!({"path": "/music"}),
    };

    let _response = dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut enum_service,
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &active,
        &noop_recent(),
        &noop_events(),
    );

    assert_eq!(enum_service.last_batch_size, Some(100), "default batch size should be 100");
}

#[test]
fn start_enumeration_passes_recursive_flag() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();
    let mut enum_service = FakeFolderEnumerationService::new();
    let active = ActiveEnumerations::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "start_enumeration".to_string(),
        payload: serde_json::json!({"path": "/music", "recursive": true}),
    };

    let _response = dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut enum_service,
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &active,
        &noop_recent(),
        &noop_events(),
    );

    assert_eq!(enum_service.last_recursive, Some(true));
}

#[test]
fn start_enumeration_defaults_recursive_false() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();
    let mut enum_service = FakeFolderEnumerationService::new();
    let active = ActiveEnumerations::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "start_enumeration".to_string(),
        payload: serde_json::json!({"path": "/music"}),
    };

    let _response = dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut enum_service,
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &active,
        &noop_recent(),
        &noop_events(),
    );

    assert_eq!(enum_service.last_recursive, Some(false));
}

#[test]
fn start_enumeration_invalid_payload_rejected() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();
    let mut enum_service = FakeFolderEnumerationService::new();
    let active = ActiveEnumerations::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "start_enumeration".to_string(),
        payload: serde_json::json!("not_an_object"),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut enum_service,
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &active,
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    assert_eq!(enum_service.start_call_count, 0);
}

#[test]
fn start_enumeration_missing_path_rejected() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();
    let mut enum_service = FakeFolderEnumerationService::new();
    let active = ActiveEnumerations::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "start_enumeration".to_string(),
        payload: serde_json::json!({}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut enum_service,
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &active,
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    assert_eq!(enum_service.start_call_count, 0);
}

#[test]
fn start_enumeration_service_error_maps_to_boundary() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();
    let mut enum_service = FakeFolderEnumerationService::new();
    enum_service.fail_start = true;
    let active = ActiveEnumerations::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "start_enumeration".to_string(),
        payload: serde_json::json!({"path": "/music"}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut enum_service,
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &active,
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    assert_eq!(response.error.unwrap().category, "Unavailable");
}

#[test]
fn cancel_enumeration_cancels_session() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();
    let mut enum_service = FakeFolderEnumerationService::new();
    let active = ActiveEnumerations::new();
    let flag = active.register("session-1");

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "cancel_enumeration".to_string(),
        payload: serde_json::json!({"session_id": "session-1"}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut enum_service,
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &active,
        &noop_recent(),
        &noop_events(),
    );

    assert!(response.ok);
    assert!(flag.load(std::sync::atomic::Ordering::Acquire), "session should be cancelled");
}

#[test]
fn cancel_enumeration_unknown_session_idempotent() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();
    let mut enum_service = FakeFolderEnumerationService::new();
    let active = ActiveEnumerations::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "cancel_enumeration".to_string(),
        payload: serde_json::json!({"session_id": "nonexistent"}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut enum_service,
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &active,
        &noop_recent(),
        &noop_events(),
    );

    assert!(response.ok, "cancelling unknown session should succeed");
}

#[test]
fn cancel_enumeration_invalid_payload_rejected() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();
    let mut enum_service = FakeFolderEnumerationService::new();
    let active = ActiveEnumerations::new();

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "cancel_enumeration".to_string(),
        payload: serde_json::json!("not_an_object"),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut enum_service,
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &active,
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    assert_eq!(response.error.unwrap().category, "InvalidInput");
}

// ── Move-to-trash command tests ───────────────────────────────────

#[test]
fn move_to_trash_command_dispatches_and_returns_results() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();
    let mut enum_service = FakeFolderEnumerationService::new();
    let active = ActiveEnumerations::new();
    let trash_service = FakeTrashService::new(Box::new(|_| {
        vec![(PathBuf::from("/music/a.wav"), Ok(())), (PathBuf::from("/music/b.wav"), Ok(()))]
    }));

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "move_to_trash".to_string(),
        payload: serde_json::json!({
            "paths": ["/music/a.wav", "/music/b.wav"]
        }),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut enum_service,
        &trash_service,
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &active,
        &noop_recent(),
        &noop_events(),
    );

    assert!(response.ok);
    let data: MoveToTrashResponse = serde_json::from_value(response.data.unwrap()).unwrap();
    assert_eq!(data.results.len(), 2);
    assert!(data.results[0].ok);
    assert!(data.results[1].ok);
}

#[test]
fn move_to_trash_reports_partial_failure() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();
    let mut enum_service = FakeFolderEnumerationService::new();
    let active = ActiveEnumerations::new();
    let trash_service = FakeTrashService::new(Box::new(|_| {
        let bad_err = ApplicationError::new(
            ErrorCategory::NotFound,
            DiagnosticContext::new(DiagnosticCode::FileOperation),
            std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
        );
        vec![
            (PathBuf::from("/music/good.wav"), Ok(())),
            (PathBuf::from("/music/bad.wav"), Err(bad_err)),
        ]
    }));

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "move_to_trash".to_string(),
        payload: serde_json::json!({
            "paths": ["/music/good.wav", "/music/bad.wav"]
        }),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut enum_service,
        &trash_service,
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &active,
        &noop_recent(),
        &noop_events(),
    );

    assert!(response.ok);
    let data: MoveToTrashResponse = serde_json::from_value(response.data.unwrap()).unwrap();
    assert_eq!(data.results.len(), 2);
    assert!(data.results[0].ok);
    assert!(!data.results[1].ok);
    assert_eq!(data.results[1].category.as_deref(), Some("NotFound"));
    assert_eq!(data.results[1].diagnostic_code.as_deref(), Some("file.operation"));
}

#[test]
fn move_to_trash_invalid_payload_rejected() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();
    let mut enum_service = FakeFolderEnumerationService::new();
    let active = ActiveEnumerations::new();
    let trash_service = FakeTrashService::new(Box::new(|_| vec![]));

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "move_to_trash".to_string(),
        payload: serde_json::json!("not_an_object"),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut enum_service,
        &trash_service,
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &active,
        &noop_recent(),
        &noop_events(),
    );

    assert!(!response.ok);
    let error = response.error.unwrap();
    assert_eq!(error.category, "InvalidInput");
    assert_eq!(error.diagnostic_code, "command.payload");
}

#[test]
fn move_to_trash_empty_paths_returns_empty() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();
    let mut enum_service = FakeFolderEnumerationService::new();
    let active = ActiveEnumerations::new();
    let trash_service = FakeTrashService::new(Box::new(|_| vec![]));

    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "move_to_trash".to_string(),
        payload: serde_json::json!({"paths": []}),
    };

    let response = dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut enum_service,
        &trash_service,
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &active,
        &noop_recent(),
        &noop_events(),
    );

    assert!(response.ok);
    let data: MoveToTrashResponse = serde_json::from_value(response.data.unwrap()).unwrap();
    assert!(data.results.is_empty());
}

// ── Folder picker command tests ────────────────────────────────────

#[test]
fn handle_pick_folder_returns_path() {
    let picker = FakeFolderPicker::returning(Some("/music/library"));

    let response = handle_pick_folder(&picker);

    assert!(response.ok);
    let data: PickFolderResponse = serde_json::from_value(response.data.unwrap()).unwrap();
    assert_eq!(data.path, Some("/music/library".to_string()));
}

#[test]
fn handle_pick_folder_returns_none_when_cancelled() {
    let picker = FakeFolderPicker::returning(None);

    let response = handle_pick_folder(&picker);

    assert!(response.ok);
    let data: PickFolderResponse = serde_json::from_value(response.data.unwrap()).unwrap();
    assert_eq!(data.path, None);
}

#[test]
fn handle_pick_folder_maps_service_error() {
    let picker = FakeFolderPicker::failing();

    let response = handle_pick_folder(&picker);

    assert!(!response.ok);
    let error = response.error.unwrap();
    assert_eq!(error.category, "PermissionDenied");
}

// ── Recent folders command tests ───────────────────────────────────

fn recent_folder_dispatch(
    command: &str,
    payload: serde_json::Value,
    recent_service: &dyn crate::recent_folders_service::RecentFoldersService,
) -> CommandResponse {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();
    let mut enum_service = FakeFolderEnumerationService::new();
    let active = ActiveEnumerations::new();
    let trash_service = FakeTrashService::new(Box::new(|_| vec![]));
    let envelope =
        CommandEnvelope { version: CURRENT_COMMAND_VERSION, command: command.to_string(), payload };
    dispatch(
        envelope,
        &mut service,
        &mut device_service,
        &mut enum_service,
        &trash_service,
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &active,
        recent_service,
        &noop_events(),
    )
}

#[test]
fn recent_folder_record_and_list_round_trip() {
    let recent_service = InMemoryRecentFoldersService::new();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_str().expect("utf8 temp path").to_string();

    let record = recent_folder_dispatch(
        "record_recent_folder",
        serde_json::json!({ "path": path }),
        &recent_service,
    );
    assert!(record.ok, "record succeeds for an existing directory");

    let list =
        recent_folder_dispatch("list_recent_folders", serde_json::json!({}), &recent_service);
    assert!(list.ok);
    let data: ListRecentFoldersResponse = serde_json::from_value(list.data.unwrap()).unwrap();
    assert_eq!(data.folders.len(), 1);
    assert_eq!(data.folders[0].path, path);
    assert!(!data.folders[0].name.is_empty());
}

#[test]
fn recent_folder_clear_empties_history() {
    let recent_service = InMemoryRecentFoldersService::new();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_str().expect("utf8 temp path").to_string();

    assert!(
        recent_folder_dispatch(
            "record_recent_folder",
            serde_json::json!({ "path": path }),
            &recent_service,
        )
        .ok
    );

    let clear =
        recent_folder_dispatch("clear_recent_folders", serde_json::json!({}), &recent_service);
    assert!(clear.ok);

    let list =
        recent_folder_dispatch("list_recent_folders", serde_json::json!({}), &recent_service);
    let data: ListRecentFoldersResponse = serde_json::from_value(list.data.unwrap()).unwrap();
    assert!(data.folders.is_empty());
}

#[test]
fn recent_folder_missing_path_returns_safe_error() {
    let recent_service = InMemoryRecentFoldersService::new();
    let secret = "/nonexistent/private-records";

    let response = recent_folder_dispatch(
        "record_recent_folder",
        serde_json::json!({ "path": secret }),
        &recent_service,
    );

    assert!(!response.ok);
    let error = response.error.unwrap();
    assert_eq!(error.category, "InvalidInput");
    assert!(!error.message.contains("private-records"), "error must not embed the path");
    assert!(!error.message.contains("/nonexistent"), "error must not embed the path");
}

#[test]
fn recent_folder_virtual_root_is_ignored() {
    let recent_service = InMemoryRecentFoldersService::new();

    let response = recent_folder_dispatch(
        "record_recent_folder",
        serde_json::json!({ "path": "computer://" }),
        &recent_service,
    );

    assert!(response.ok);
    let list =
        recent_folder_dispatch("list_recent_folders", serde_json::json!({}), &recent_service);
    let data: ListRecentFoldersResponse = serde_json::from_value(list.data.unwrap()).unwrap();
    assert!(data.folders.is_empty());
}

#[test]
fn recent_folder_invalid_payload_rejected() {
    let recent_service = InMemoryRecentFoldersService::new();

    let response = recent_folder_dispatch(
        "record_recent_folder",
        serde_json::json!("not_an_object"),
        &recent_service,
    );

    assert!(!response.ok);
    let error = response.error.unwrap();
    assert_eq!(error.category, "InvalidInput");
    assert_eq!(error.diagnostic_code, "command.payload");
}

// ── Rename file command tests ─────────────────────────────────────

fn rename_file_dispatch(
    payload: serde_json::Value,
    playback: &mut FakePlaybackService,
    rename: &dyn crate::rename_service::RenameService,
) -> CommandResponse {
    let mut device_service = FakeAudioDeviceService::new();
    let mut enum_service = FakeFolderEnumerationService::new();
    let active = ActiveEnumerations::new();
    let trash_service = FakeTrashService::new(Box::new(|_| vec![]));
    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "rename_file".to_string(),
        payload,
    };
    dispatch(
        envelope,
        playback,
        &mut device_service,
        &mut enum_service,
        &trash_service,
        rename,
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &active,
        &noop_recent(),
        &noop_events(),
    )
}

#[test]
fn rename_file_returns_new_path_and_reconciles_playing_file() {
    let mut playback = FakePlaybackService::new();
    playback.reconcile_path_result = Some(true);
    let rename = FakeRenameService::new(Box::new(|path, name| {
        assert_eq!(path, "/music/track.wav");
        assert_eq!(name, "renamed.wav");
        Ok(RenameOutcome {
            old_path: "/music/track.wav".to_string(),
            new_path: "/music/renamed.wav".to_string(),
        })
    }));

    let response = rename_file_dispatch(
        serde_json::json!({ "path": "/music/track.wav", "new_name": "renamed.wav" }),
        &mut playback,
        &rename,
    );

    assert!(response.ok, "rename should succeed");
    let data: RenameFileResponse = serde_json::from_value(response.data.unwrap()).unwrap();
    assert_eq!(data.old_path, "/music/track.wav");
    assert_eq!(data.new_path, "/music/renamed.wav");
    assert!(data.was_playing, "renamed file is the playing file");
    assert_eq!(playback.reconcile_path_call_count, 1);
    assert_eq!(playback.last_reconcile_old_path.as_deref(), Some("/music/track.wav"));
    assert_eq!(playback.last_reconcile_new_path.as_deref(), Some("/music/renamed.wav"));
}

#[test]
fn rename_file_reports_when_other_file_is_playing() {
    let mut playback = FakePlaybackService::new();
    playback.reconcile_path_result = Some(false);
    let rename = FakeRenameService::new(Box::new(|_, _| {
        Ok(RenameOutcome {
            old_path: "/music/track.wav".to_string(),
            new_path: "/music/renamed.wav".to_string(),
        })
    }));

    let response = rename_file_dispatch(
        serde_json::json!({ "path": "/music/track.wav", "new_name": "renamed.wav" }),
        &mut playback,
        &rename,
    );

    assert!(response.ok);
    let data: RenameFileResponse = serde_json::from_value(response.data.unwrap()).unwrap();
    assert!(!data.was_playing);
}

#[test]
fn rename_file_service_error_maps_to_boundary() {
    let mut playback = FakePlaybackService::new();
    let rename = FakeRenameService::new(Box::new(|_, _| {
        Err(ApplicationError::new(
            ErrorCategory::Conflict,
            DiagnosticContext::new(DiagnosticCode::FileOperation),
            std::io::Error::new(std::io::ErrorKind::AlreadyExists, "collision"),
        ))
    }));

    let response = rename_file_dispatch(
        serde_json::json!({ "path": "/music/track.wav", "new_name": "existing.wav" }),
        &mut playback,
        &rename,
    );

    assert!(!response.ok);
    let error = response.error.unwrap();
    assert_eq!(error.category, "Conflict");
    assert_eq!(error.diagnostic_code, "file.operation");
    assert_eq!(playback.reconcile_path_call_count, 0, "no reconciliation after failure");
}

#[test]
fn rename_file_playback_error_maps_to_boundary() {
    let mut playback = FakePlaybackService::new();
    playback.fail_with = Some(ErrorCategory::Unavailable);
    let rename = FakeRenameService::new(Box::new(|_, _| {
        Ok(RenameOutcome {
            old_path: "/music/track.wav".to_string(),
            new_path: "/music/renamed.wav".to_string(),
        })
    }));

    let response = rename_file_dispatch(
        serde_json::json!({ "path": "/music/track.wav", "new_name": "renamed.wav" }),
        &mut playback,
        &rename,
    );

    assert!(!response.ok);
    let error = response.error.unwrap();
    assert_eq!(error.category, "Unavailable");
}

#[test]
fn rename_file_invalid_payload_rejected() {
    let mut playback = FakePlaybackService::new();
    let rename = FakeRenameService::new(Box::new(|_, _| {
        Ok(RenameOutcome {
            old_path: "/music/track.wav".to_string(),
            new_path: "/music/renamed.wav".to_string(),
        })
    }));

    let response = rename_file_dispatch(serde_json::json!("not_an_object"), &mut playback, &rename);

    assert!(!response.ok);
    let error = response.error.unwrap();
    assert_eq!(error.category, "InvalidInput");
    assert_eq!(error.diagnostic_code, "command.payload");
    assert_eq!(playback.reconcile_path_call_count, 0, "no reconciliation on bad payload");
}

// ── Move files command tests ─────────────────────────────────────

/// Wraps a [`FakeMoveService`] and records every cancelled session id.
struct RecordingMoveService {
    inner: FakeMoveService,
    cancelled: std::sync::Mutex<Vec<String>>,
}

impl RecordingMoveService {
    fn new(inner: FakeMoveService) -> Self {
        Self { inner, cancelled: std::sync::Mutex::new(Vec::new()) }
    }

    fn cancelled_ids(&self) -> Vec<String> {
        self.cancelled.lock().expect("cancelled mutex poisoned").clone()
    }
}

impl MoveService for RecordingMoveService {
    fn start_move(
        &self,
        paths: Vec<String>,
        target_dir: String,
        events: Arc<dyn PlaybackEventEmitter>,
    ) -> Result<String, ApplicationError> {
        self.inner.start_move(paths, target_dir, events)
    }

    fn cancel_move(&self, session_id: &str) {
        self.cancelled.lock().expect("cancelled mutex poisoned").push(session_id.to_string());
    }
}

fn move_dispatch(payload: serde_json::Value, service: &dyn MoveService) -> CommandResponse {
    let mut playback = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();
    let mut enum_service = FakeFolderEnumerationService::new();
    let active = ActiveEnumerations::new();
    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: match payload {
            serde_json::Value::Object(ref object) => {
                if object.contains_key("paths") {
                    "start_move_files".to_string()
                } else {
                    "cancel_move_files".to_string()
                }
            },
            _ => "start_move_files".to_string(),
        },
        payload,
    };
    dispatch(
        envelope,
        &mut playback,
        &mut device_service,
        &mut enum_service,
        &noop_trash(),
        &noop_rename(),
        service,
        &noop_copy(),
        &noop_external(),
        &noop_drag_out(),
        &active,
        &noop_recent(),
        &noop_events(),
    )
}

#[test]
fn start_move_files_returns_session_id_and_forwards_args() {
    let move_service = FakeMoveService::new(Box::new(|paths, target_dir, _| {
        assert_eq!(paths, vec!["/music/a.wav", "/music/b.wav"]);
        assert_eq!(target_dir, "/library");
        Ok("move-7".to_string())
    }));

    let response = move_dispatch(
        serde_json::json!({
            "paths": ["/music/a.wav", "/music/b.wav"],
            "target_dir": "/library"
        }),
        &move_service,
    );

    assert!(response.ok);
    let data: StartMoveFilesResponse = serde_json::from_value(response.data.unwrap()).unwrap();
    assert_eq!(data.session_id, "move-7");
}

#[test]
fn start_move_files_service_error_maps_to_boundary() {
    let move_service = FakeMoveService::new(Box::new(|_, _, _| {
        Err(ApplicationError::new(
            ErrorCategory::InvalidInput,
            DiagnosticContext::new(DiagnosticCode::FileOperation),
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid target"),
        ))
    }));

    let response = move_dispatch(
        serde_json::json!({ "paths": ["/music/a.wav"], "target_dir": "/missing" }),
        &move_service,
    );

    assert!(!response.ok);
    let error = response.error.unwrap();
    assert_eq!(error.category, "InvalidInput");
    assert_eq!(error.diagnostic_code, "file.operation");
}

#[test]
fn start_move_files_invalid_payload_rejected() {
    let move_service = FakeMoveService::new(Box::new(|_, _, _| Ok("move-1".to_string())));

    let response = move_dispatch(serde_json::json!("not_an_object"), &move_service);

    assert!(!response.ok);
    let error = response.error.unwrap();
    assert_eq!(error.category, "InvalidInput");
    assert_eq!(error.diagnostic_code, "command.payload");
}

#[test]
fn cancel_move_files_forwards_session_to_service() {
    let move_service = RecordingMoveService::new(FakeMoveService::new(Box::new(|_, _, _| {
        Ok("move-1".to_string())
    })));

    let response = move_dispatch(serde_json::json!({ "session_id": "move-9" }), &move_service);

    assert!(response.ok);
    assert_eq!(move_service.cancelled_ids(), vec!["move-9"]);
}

#[test]
fn cancel_move_files_invalid_payload_rejected() {
    let move_service = RecordingMoveService::new(FakeMoveService::new(Box::new(|_, _, _| {
        Ok("move-1".to_string())
    })));

    let response = move_dispatch(serde_json::json!("not_an_object"), &move_service);

    assert!(!response.ok);
    let error = response.error.unwrap();
    assert_eq!(error.diagnostic_code, "command.payload");
    assert!(move_service.cancelled_ids().is_empty(), "service must not be called");
}

// ── Copy files command tests ─────────────────────────────────────

/// Wraps a [`FakeCopyService`] and records every cancelled session id.
struct RecordingCopyService {
    inner: FakeCopyService,
    cancelled: std::sync::Mutex<Vec<String>>,
}

impl RecordingCopyService {
    fn new(inner: FakeCopyService) -> Self {
        Self { inner, cancelled: std::sync::Mutex::new(Vec::new()) }
    }

    fn cancelled_ids(&self) -> Vec<String> {
        self.cancelled.lock().expect("cancelled mutex poisoned").clone()
    }
}

impl CopyService for RecordingCopyService {
    fn start_copy(
        &self,
        paths: Vec<String>,
        target_dir: String,
        events: Arc<dyn PlaybackEventEmitter>,
    ) -> Result<String, ApplicationError> {
        self.inner.start_copy(paths, target_dir, events)
    }

    fn cancel_copy(&self, session_id: &str) {
        self.cancelled.lock().expect("cancelled mutex poisoned").push(session_id.to_string());
    }
}

fn copy_dispatch(payload: serde_json::Value, service: &dyn CopyService) -> CommandResponse {
    let mut playback = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();
    let mut enum_service = FakeFolderEnumerationService::new();
    let active = ActiveEnumerations::new();
    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: match payload {
            serde_json::Value::Object(ref object) => {
                if object.contains_key("paths") {
                    "start_copy_files".to_string()
                } else {
                    "cancel_copy_files".to_string()
                }
            },
            _ => "start_copy_files".to_string(),
        },
        payload,
    };
    dispatch(
        envelope,
        &mut playback,
        &mut device_service,
        &mut enum_service,
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        service,
        &noop_external(),
        &noop_drag_out(),
        &active,
        &noop_recent(),
        &noop_events(),
    )
}

#[test]
fn start_copy_files_returns_session_id_and_forwards_args() {
    let copy_service = FakeCopyService::new(Box::new(|paths, target_dir, _| {
        assert_eq!(paths, vec!["/music/a.wav", "/music/b.wav"]);
        assert_eq!(target_dir, "/library");
        Ok("copy-7".to_string())
    }));

    let response = copy_dispatch(
        serde_json::json!({
            "paths": ["/music/a.wav", "/music/b.wav"],
            "target_dir": "/library"
        }),
        &copy_service,
    );

    assert!(response.ok);
    let data: StartCopyFilesResponse = serde_json::from_value(response.data.unwrap()).unwrap();
    assert_eq!(data.session_id, "copy-7");
}

#[test]
fn start_copy_files_service_error_maps_to_boundary() {
    let copy_service = FakeCopyService::new(Box::new(|_, _, _| {
        Err(ApplicationError::new(
            ErrorCategory::InvalidInput,
            DiagnosticContext::new(DiagnosticCode::FileOperation),
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid target"),
        ))
    }));

    let response = copy_dispatch(
        serde_json::json!({ "paths": ["/music/a.wav"], "target_dir": "/missing" }),
        &copy_service,
    );

    assert!(!response.ok);
    let error = response.error.unwrap();
    assert_eq!(error.category, "InvalidInput");
    assert_eq!(error.diagnostic_code, "file.operation");
}

#[test]
fn start_copy_files_invalid_payload_rejected() {
    let copy_service = FakeCopyService::new(Box::new(|_, _, _| Ok("copy-1".to_string())));

    let response = copy_dispatch(serde_json::json!("not_an_object"), &copy_service);

    assert!(!response.ok);
    let error = response.error.unwrap();
    assert_eq!(error.category, "InvalidInput");
    assert_eq!(error.diagnostic_code, "command.payload");
}

#[test]
fn cancel_copy_files_forwards_session_to_service() {
    let copy_service = RecordingCopyService::new(FakeCopyService::new(Box::new(|_, _, _| {
        Ok("copy-1".to_string())
    })));

    let response = copy_dispatch(serde_json::json!({ "session_id": "copy-9" }), &copy_service);

    assert!(response.ok);
    assert_eq!(copy_service.cancelled_ids(), vec!["copy-9"]);
}

#[test]
fn cancel_copy_files_invalid_payload_rejected() {
    let copy_service = RecordingCopyService::new(FakeCopyService::new(Box::new(|_, _, _| {
        Ok("copy-1".to_string())
    })));

    let response = copy_dispatch(serde_json::json!("not_an_object"), &copy_service);

    assert!(!response.ok);
    let error = response.error.unwrap();
    assert_eq!(error.diagnostic_code, "command.payload");
    assert!(copy_service.cancelled_ids().is_empty(), "service must not be called");
}

fn external_dispatch(
    command: &str,
    payload: serde_json::Value,
    service: &dyn ExternalService,
) -> CommandResponse {
    let mut playback = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();
    let mut enum_service = FakeFolderEnumerationService::new();
    let active = ActiveEnumerations::new();
    let envelope =
        CommandEnvelope { version: CURRENT_COMMAND_VERSION, command: command.to_string(), payload };
    dispatch(
        envelope,
        &mut playback,
        &mut device_service,
        &mut enum_service,
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        service,
        &noop_drag_out(),
        &active,
        &noop_recent(),
        &noop_events(),
    )
}

#[test]
fn reveal_file_command_dispatches_and_returns_ok() {
    let external = FakeExternalService::new(
        Box::new(|path| {
            assert_eq!(path, "/music/a.wav");
            Ok(())
        }),
        Box::new(|_| Ok(())),
    );
    let response =
        external_dispatch("reveal_file", serde_json::json!({ "path": "/music/a.wav" }), &external);
    assert!(response.ok);
    assert!(response.error.is_none());
}

#[test]
fn reveal_file_service_error_maps_to_boundary() {
    let external = FakeExternalService::new(
        Box::new(|_| {
            Err(ApplicationError::new(
                ErrorCategory::NotFound,
                DiagnosticContext::new(DiagnosticCode::FileOperation),
                std::io::Error::other("missing"),
            ))
        }),
        Box::new(|_| Ok(())),
    );
    let response = external_dispatch(
        "reveal_file",
        serde_json::json!({ "path": "/music/missing.wav" }),
        &external,
    );
    assert!(!response.ok);
    let error = response.error.unwrap();
    assert_eq!(error.category, "NotFound");
    assert_eq!(error.diagnostic_code, "file.operation");
}

#[test]
fn reveal_file_invalid_payload_rejected() {
    let external = FakeExternalService::new(Box::new(|_| Ok(())), Box::new(|_| Ok(())));
    let response = external_dispatch("reveal_file", serde_json::json!("not_an_object"), &external);
    assert!(!response.ok);
    assert_eq!(response.error.unwrap().diagnostic_code, "command.payload");
}

#[test]
fn open_with_command_dispatches_and_returns_ok() {
    let external = FakeExternalService::new(
        Box::new(|_| Ok(())),
        Box::new(|path| {
            assert_eq!(path, "/music/a.wav");
            Ok(())
        }),
    );
    let response =
        external_dispatch("open_with", serde_json::json!({ "path": "/music/a.wav" }), &external);
    assert!(response.ok);
    assert!(response.error.is_none());
}

#[test]
fn open_with_unsupported_platform_maps_to_boundary() {
    let external = FakeExternalService::new(
        Box::new(|_| Ok(())),
        Box::new(|_| {
            Err(ApplicationError::new(
                ErrorCategory::Unsupported,
                DiagnosticContext::new(DiagnosticCode::FileOperation),
                std::io::Error::other("unsupported"),
            ))
        }),
    );
    let response =
        external_dispatch("open_with", serde_json::json!({ "path": "/music/a.wav" }), &external);
    assert!(!response.ok);
    let error = response.error.unwrap();
    assert_eq!(error.category, "Unsupported");
    assert_eq!(error.diagnostic_code, "file.operation");
}

#[test]
fn open_with_invalid_payload_rejected() {
    let external = FakeExternalService::new(Box::new(|_| Ok(())), Box::new(|_| Ok(())));
    let response = external_dispatch("open_with", serde_json::json!("not_an_object"), &external);
    assert!(!response.ok);
    assert_eq!(response.error.unwrap().diagnostic_code, "command.payload");
}

fn drag_out_dispatch(payload: serde_json::Value, service: &dyn DragOutService) -> CommandResponse {
    let mut playback = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();
    let mut enum_service = FakeFolderEnumerationService::new();
    let active = ActiveEnumerations::new();
    let envelope = CommandEnvelope {
        version: CURRENT_COMMAND_VERSION,
        command: "drag_out".to_string(),
        payload,
    };
    dispatch(
        envelope,
        &mut playback,
        &mut device_service,
        &mut enum_service,
        &noop_trash(),
        &noop_rename(),
        &noop_move(),
        &noop_copy(),
        &noop_external(),
        service,
        &active,
        &noop_recent(),
        &noop_events(),
    )
}

#[test]
fn drag_out_command_dispatches_and_returns_ok() {
    let drag = FakeDragOutService::new(Box::new(|paths| {
        assert_eq!(paths, vec!["/music/a.wav".to_string(), "/music/b.wav".to_string()]);
        Ok(())
    }));
    let response =
        drag_out_dispatch(serde_json::json!({ "paths": ["/music/a.wav", "/music/b.wav"] }), &drag);
    assert!(response.ok);
    assert!(response.error.is_none());
}

#[test]
fn drag_out_missing_target_maps_to_boundary() {
    let drag = FakeDragOutService::new(Box::new(|_| {
        Err(ApplicationError::new(
            ErrorCategory::NotFound,
            DiagnosticContext::new(DiagnosticCode::FileOperation),
            std::io::Error::other("missing"),
        ))
    }));
    let response = drag_out_dispatch(serde_json::json!({ "paths": ["/music/missing.wav"] }), &drag);
    assert!(!response.ok);
    let error = response.error.unwrap();
    assert_eq!(error.category, "NotFound");
    assert_eq!(error.diagnostic_code, "file.operation");
}

#[test]
fn drag_out_cancellation_maps_to_boundary() {
    let drag = FakeDragOutService::new(Box::new(|_| {
        Err(ApplicationError::new(
            ErrorCategory::Cancelled,
            DiagnosticContext::new(DiagnosticCode::FileOperation),
            std::io::Error::other("cancelled"),
        ))
    }));
    let response = drag_out_dispatch(serde_json::json!({ "paths": ["/music/a.wav"] }), &drag);
    assert!(!response.ok);
    let error = response.error.unwrap();
    assert_eq!(error.category, "Cancelled");
    assert_eq!(error.diagnostic_code, "file.operation");
}

#[test]
fn drag_out_invalid_payload_rejected() {
    let drag = FakeDragOutService::new(Box::new(|_| Ok(())));
    let response = drag_out_dispatch(serde_json::json!("not_an_object"), &drag);
    assert!(!response.ok);
    assert_eq!(response.error.unwrap().diagnostic_code, "command.payload");
}
