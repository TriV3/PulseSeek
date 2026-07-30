use pulseseek_audio_cpal::CpalAudioOutput;
use pulseseek_domain::audio_output::{AudioOutput, DeviceId};
use pulseseek_domain::error::{ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory};

use crate::audio_device_service::{AudioDeviceService, DeviceInfoData};

/// Tauri-facing audio-device service backed by the native cpal adapter.
pub struct NativeAudioDeviceService {
    output: CpalAudioOutput,
}

impl NativeAudioDeviceService {
    pub fn new() -> Self {
        Self { output: CpalAudioOutput::new() }
    }

    fn unavailable(message: &str) -> ApplicationError {
        ApplicationError::new(
            ErrorCategory::Unavailable,
            DiagnosticContext::new(DiagnosticCode::AudioOutput),
            std::io::Error::other(message),
        )
    }
}

impl Default for NativeAudioDeviceService {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioDeviceService for NativeAudioDeviceService {
    fn list_devices(&self) -> Result<Vec<DeviceInfoData>, ApplicationError> {
        self.output
            .list_devices()
            .map(|devices| {
                devices
                    .into_iter()
                    .map(|device| DeviceInfoData {
                        id: device.id.as_str().to_string(),
                        name: device.name,
                    })
                    .collect()
            })
            .map_err(|error| Self::unavailable(&error.to_string()))
    }

    fn current_device(&self) -> Result<Option<DeviceInfoData>, ApplicationError> {
        let Some(device_id) = self.output.current_device() else {
            return Ok(None);
        };
        let device_id_string = device_id.as_str().to_string();
        Ok(self.list_devices()?.into_iter().find(|device| device.id == device_id_string))
    }

    fn select_device(&mut self, device_id: &str) -> Result<(), ApplicationError> {
        self.output
            .open(&DeviceId::new(device_id))
            .map_err(|error| Self::unavailable(&error.to_string()))
    }

    fn is_device_lost(&self) -> bool {
        self.output.is_device_lost()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_service_starts_without_selected_device() {
        let service = NativeAudioDeviceService::new();
        assert!(service.output.current_device().is_none());
    }
}
