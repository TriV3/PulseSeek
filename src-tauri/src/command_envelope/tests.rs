use std::path::PathBuf;

use pulseseek_domain::error::{DiagnosticCode, DiagnosticContext, ErrorCategory};

use super::*;
use crate::audio_device_service::{DeviceInfoData, FakeAudioDeviceService};
use crate::dialog_service::FakeFolderPicker;
use crate::folder_enumeration_service::{ActiveEnumerations, FakeFolderEnumerationService};
use crate::playback_events::{FakeEventEmitter, NoopEventEmitter};
use crate::playback_service::FakePlaybackService;
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
        &noop_active(),
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
            &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
        &noop_events(),
    );

    assert!(response.ok);
    assert_eq!(device_service.select_call_count, 1);
    assert_eq!(device_service.last_select_id, Some("hdmi".to_string()));
    assert_eq!(device_service.current.as_ref().map(|d| d.id.as_str()), Some("hdmi"));
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
        &noop_active(),
        &noop_events(),
    );

    assert!(!response.ok);
    assert_eq!(device_service.select_call_count, 0);
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
        &noop_active(),
        &noop_events(),
    );

    assert!(!response.ok);
    assert_eq!(device_service.select_call_count, 0);
}

#[test]
fn select_device_service_error_maps_to_boundary() {
    let mut service = FakePlaybackService::new();
    let mut device_service = FakeAudioDeviceService::new();
    device_service.fail_select = true;

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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &noop_active(),
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
        &active,
        &noop_events(),
    );

    assert!(response.ok);
    assert_eq!(enum_service.start_call_count, 1);
    assert_eq!(enum_service.last_path, Some("/music".to_string()));
    let data = response.data.expect("should have data");
    assert_eq!(data["session_id"], "test-session-001");
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
        &active,
        &noop_events(),
    );

    assert_eq!(enum_service.last_batch_size, Some(100), "default batch size should be 100");
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
        &active,
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
        &active,
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
        &active,
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
        &active,
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
        &active,
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
        &active,
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
        &active,
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
        &active,
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
        &active,
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
        &active,
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
