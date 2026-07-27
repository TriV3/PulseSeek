use std::collections::HashSet;

use cpal::traits::{DeviceTrait, HostTrait};

use pulseseek_domain::audio_output::{
    AudioOutput, AudioOutputError, DeviceId, DeviceInfo, StreamState,
};
use pulseseek_domain::error::{DiagnosticCode, DiagnosticContext};
use pulseseek_domain::playback::volume::{Gain, Volume};

/// A cpal-based audio output adapter.
pub struct CpalAudioOutput {
    current_device: Option<DeviceId>,
    state: StreamState,
    #[allow(dead_code)]
    volume: Volume,
    #[allow(dead_code)]
    device_lost: bool,
}

impl CpalAudioOutput {
    /// Creates a new cpal audio output adapter.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for CpalAudioOutput {
    fn default() -> Self {
        Self {
            current_device: None,
            state: StreamState::Stopped,
            volume: Volume::new(Gain::new(1.0)),
            device_lost: false,
        }
    }
}

impl CpalAudioOutput {
    /// Maps a cpal device to a domain DeviceInfo.
    fn device_info(device: &cpal::Device) -> Result<DeviceInfo, AudioOutputError> {
        let name = device.name().map_err(|e| {
            AudioOutputError::new(DiagnosticContext::new(DiagnosticCode::AudioOutput), e)
        })?;

        let id = DeviceId::new(name.clone());

        // Query supported output configs.
        let mut max_channels: u16 = 0;
        let mut sample_rates: Vec<u32> = Vec::new();
        let mut seen_rates: HashSet<u32> = HashSet::new();

        if let Ok(configs) = device.supported_output_configs() {
            for cfg in configs {
                let ch = cfg.channels();
                if ch > max_channels {
                    max_channels = ch;
                }
                let min_rate = cfg.min_sample_rate().0;
                let max_rate = cfg.max_sample_rate().0;
                for rate in &[min_rate, max_rate] {
                    if seen_rates.insert(*rate) {
                        sample_rates.push(*rate);
                    }
                }
            }
        }

        sample_rates.sort();

        Ok(DeviceInfo { id, name, max_channels, sample_rates })
    }
}

impl AudioOutput for CpalAudioOutput {
    fn list_devices(&self) -> Result<Vec<DeviceInfo>, AudioOutputError> {
        let host = cpal::default_host();
        let mut devices: Vec<DeviceInfo> = Vec::new();
        let mut seen_ids: HashSet<String> = HashSet::new();

        // Add default device first if available.
        if let Some(default) = host.default_output_device() {
            if let Ok(info) = Self::device_info(&default) {
                seen_ids.insert(info.id.as_str().to_string());
                devices.push(info);
            }
        }

        // Add remaining output devices.
        if let Ok(outputs) = host.output_devices() {
            for device in outputs {
                if let Ok(info) = Self::device_info(&device) {
                    if seen_ids.insert(info.id.as_str().to_string()) {
                        devices.push(info);
                    }
                }
            }
        }

        Ok(devices)
    }

    fn open(&mut self, _device: &DeviceId) -> Result<(), AudioOutputError> {
        // Will be implemented in PR-023.
        unimplemented!("device selection not yet implemented")
    }

    fn play(&mut self) -> Result<(), AudioOutputError> {
        unimplemented!("stream start not yet implemented")
    }

    fn pause(&mut self) -> Result<(), AudioOutputError> {
        unimplemented!("stream pause not yet implemented")
    }

    fn stop(&mut self) -> Result<(), AudioOutputError> {
        unimplemented!("stream stop not yet implemented")
    }

    fn set_volume(&mut self, _volume: Volume) -> Result<(), AudioOutputError> {
        unimplemented!("volume control not yet implemented")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_name_to_id_uses_name() {
        let id = DeviceId::new("Test Speaker");
        assert_eq!(id.as_str(), "Test Speaker");
    }

    #[test]
    fn enumerate_returns_devices_with_id_and_name() {
        let output = CpalAudioOutput::new();
        let devices = output.list_devices().expect("list_devices should succeed");
        assert!(!devices.is_empty(), "at least one output device should be available");

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
            if let Ok(name) = default.name() {
                let found = devices.iter().any(|d| d.name == name);
                assert!(found, "default device '{}' should be in device list", name);
            }
        }
    }
}
