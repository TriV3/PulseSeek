use pulseseek_domain::audio_output::{
    AudioOutput, AudioOutputError, DeviceId, DeviceInfo, StreamState,
};
use pulseseek_domain::error::{DiagnosticCode, DiagnosticContext, ErrorContract};
use pulseseek_domain::playback::volume::{Gain, Mute, Volume};

/// A fake audio output adapter for contract testing.
struct FakeAudioOutput {
    current_device: Option<DeviceId>,
    state: StreamState,
    volume: Volume,
    device_lost: bool,
    devices: Vec<DeviceInfo>,
}

impl FakeAudioOutput {
    fn new() -> Self {
        Self {
            current_device: None,
            state: StreamState::Stopped,
            volume: Volume::new(Gain::new(1.0)),
            device_lost: false,
            devices: vec![
                DeviceInfo {
                    id: DeviceId::new("default"),
                    name: "Default Output".to_string(),
                    max_channels: 2,
                    sample_rates: vec![44100, 48000],
                },
                DeviceInfo {
                    id: DeviceId::new("hdmi"),
                    name: "HDMI Audio".to_string(),
                    max_channels: 8,
                    sample_rates: vec![44100, 48000, 96000],
                },
            ],
        }
    }
}

impl AudioOutput for FakeAudioOutput {
    fn list_devices(&self) -> Result<Vec<DeviceInfo>, AudioOutputError> {
        Ok(self.devices.clone())
    }

    fn open(&mut self, device: &DeviceId) -> Result<(), AudioOutputError> {
        if self.devices.iter().any(|d| d.id == *device) {
            self.current_device = Some(device.clone());
            Ok(())
        } else {
            Err(AudioOutputError::new(
                DiagnosticContext::new(DiagnosticCode::AudioOutput),
                std::io::Error::other("unknown device"),
            ))
        }
    }

    fn play(&mut self) -> Result<(), AudioOutputError> {
        if self.current_device.is_some() {
            self.state = StreamState::Playing;
            Ok(())
        } else {
            Err(AudioOutputError::new(
                DiagnosticContext::new(DiagnosticCode::AudioOutput),
                std::io::Error::other("no device open"),
            ))
        }
    }

    fn pause(&mut self) -> Result<(), AudioOutputError> {
        self.state = StreamState::Paused;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), AudioOutputError> {
        self.state = StreamState::Stopped;
        self.current_device = None;
        Ok(())
    }

    fn set_volume(&mut self, volume: Volume) -> Result<(), AudioOutputError> {
        self.volume = volume;
        Ok(())
    }

    fn is_device_lost(&self) -> bool {
        self.device_lost
    }

    fn current_device(&self) -> Option<DeviceId> {
        self.current_device.clone()
    }

    fn state(&self) -> StreamState {
        self.state
    }
}

#[test]
fn fake_list_devices_returns_two() {
    let output = FakeAudioOutput::new();
    let devices = output.list_devices().unwrap();
    assert_eq!(devices.len(), 2);
}

#[test]
fn fake_open_then_current_device() {
    let mut output = FakeAudioOutput::new();
    let id = DeviceId::new("default");
    output.open(&id).unwrap();
    assert_eq!(output.current_device(), Some(id));
}

#[test]
fn fake_open_unknown_device_fails() {
    let mut output = FakeAudioOutput::new();
    let id = DeviceId::new("nonexistent");
    assert!(output.open(&id).is_err());
}

#[test]
fn fake_play_transitions_to_playing() {
    let mut output = FakeAudioOutput::new();
    output.open(&DeviceId::new("default")).unwrap();
    output.play().unwrap();
    assert_eq!(output.state(), StreamState::Playing);
}

#[test]
fn fake_pause_transitions_to_paused() {
    let mut output = FakeAudioOutput::new();
    output.open(&DeviceId::new("default")).unwrap();
    output.play().unwrap();
    output.pause().unwrap();
    assert_eq!(output.state(), StreamState::Paused);
}

#[test]
fn fake_stop_transitions_to_stopped() {
    let mut output = FakeAudioOutput::new();
    output.open(&DeviceId::new("default")).unwrap();
    output.play().unwrap();
    output.stop().unwrap();
    assert_eq!(output.state(), StreamState::Stopped);
    assert_eq!(output.current_device(), None);
}

#[test]
fn fake_set_volume_affects_internal_state() {
    let mut output = FakeAudioOutput::new();
    let vol = Volume::muted();
    output.set_volume(vol).unwrap();
    assert_eq!(output.volume.mute(), Mute::On);
    assert_eq!(output.volume.effective_gain(), 0.0);
}

#[test]
fn fake_device_not_lost_by_default() {
    let output = FakeAudioOutput::new();
    assert!(!output.is_device_lost());
}

#[test]
fn fake_device_info_contains_id_and_name() {
    let output = FakeAudioOutput::new();
    let devices = output.list_devices().unwrap();
    for d in &devices {
        assert!(!d.id.as_str().is_empty(), "device id should not be empty");
        assert!(!d.name.is_empty(), "device name should not be empty");
    }
}

#[test]
fn audio_output_error_implements_error_contract() {
    let err = AudioOutputError::new(
        DiagnosticContext::new(DiagnosticCode::AudioOutput),
        std::io::Error::other("test error"),
    );
    let desc = err.user_descriptor();
    assert!(!desc.message().is_empty(), "error should have a safe message");
}
