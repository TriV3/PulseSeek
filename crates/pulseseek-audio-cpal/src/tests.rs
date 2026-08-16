use std::sync::atomic::Ordering;

use super::*;
use pulseseek_domain::audio_output::{AudioOutput, DeviceId, StreamState};
use pulseseek_domain::playback::volume::{Gain, Volume};

#[test]
fn device_name_to_id_uses_name() {
    let id = DeviceId::new("Test Speaker");
    assert_eq!(id.as_str(), "Test Speaker");
}

#[test]
fn enumerate_returns_devices_with_id_and_name() {
    let output = CpalAudioOutput::new();
    let devices = output.list_devices().expect("list_devices should succeed");
    if devices.is_empty() {
        return;
    }

    for d in &devices {
        assert!(!d.id.as_str().is_empty(), "device id should not be empty");
        assert!(!d.name.is_empty(), "device name should not be empty");
    }
}

#[test]
fn enumerate_includes_default_device() {
    let output = CpalAudioOutput::new();
    let devices = output.list_devices().expect("list_devices should succeed");

    let host = cpal::default_host();
    if let Some(default) = host.default_output_device() {
        if let Ok(name) = default.description().map(|description| description.name().to_string()) {
            let found = devices.iter().any(|d| d.name == name);
            assert!(found, "default device '{}' should be in device list", name);
        }
    }
}

#[test]
fn open_known_device_sets_current() {
    let mut output = CpalAudioOutput::new();
    let devices = output.list_devices().expect("list_devices should succeed");
    if devices.is_empty() {
        return;
    }

    let id = devices[0].id.clone();
    output.open(&id).expect("open should succeed for known device");
    assert_eq!(output.current_device(), Some(id));
}

#[test]
fn open_unknown_device_falls_back_to_default() {
    let mut output = CpalAudioOutput::new();
    let unknown = DeviceId::new("__nonexistent_device__");

    let host = cpal::default_host();
    if host.default_output_device().is_some() {
        output.open(&unknown).expect("open should fall back to default");
        assert!(output.current_device().is_some(), "should have a device after fallback");
    } else {
        assert!(output.open(&unknown).is_err(), "open should fail without fallback");
    }
}

#[test]
fn play_requires_an_open_stream() {
    let mut output = CpalAudioOutput::new();

    assert!(output.play().is_err());
    assert_eq!(output.state(), StreamState::Stopped);
}

#[test]
fn stop_without_stream_is_idempotent() {
    let mut output = CpalAudioOutput::new();

    output.stop().expect("stop should be safe before stream creation");
    output.stop().expect("stop should remain idempotent");
    assert_eq!(output.state(), StreamState::Stopped);
}

#[test]
fn set_volume_updates_callback_gain_without_creating_stream() {
    let mut output = CpalAudioOutput::new();

    output.set_volume(Volume::new(Gain::new(0.25))).expect("volume update should succeed");

    assert_eq!(f32::from_bits(output.volume_gain.load(Ordering::Relaxed)), 0.25);
    assert!(output.stream.is_none(), "volume update must not create a stream");
}

#[test]
fn output_sample_rate_requires_open_device() {
    let output = CpalAudioOutput::new();
    assert!(output.output_sample_rate().is_err());
}

#[test]
fn device_loss_pauses_output_and_is_idempotent() {
    let status = StreamStatus::new();
    status.set_playing();

    status.mark_device_lost();
    status.mark_device_lost();

    assert!(status.is_device_lost());
    assert_eq!(status.state(), StreamState::Paused);
}

#[test]
fn opening_recovered_device_clears_loss_and_stops_output() {
    let status = StreamStatus::new();
    status.set_playing();
    status.mark_device_lost();

    status.reset_after_open();

    assert!(!status.is_device_lost());
    assert_eq!(status.state(), StreamState::Stopped);
}

#[test]
fn play_rejects_lost_device_instead_of_restoring_playing_state() {
    let mut output = CpalAudioOutput::new();
    output.status.set_playing();
    output.status.mark_device_lost();

    assert!(output.play().is_err());
    assert_eq!(output.state(), StreamState::Paused);
}
