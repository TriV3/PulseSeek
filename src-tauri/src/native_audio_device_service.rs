use std::sync::{Arc, Mutex};

use pulseseek_audio_cpal::CpalAudioOutput;
use pulseseek_domain::audio_output::{AudioOutput, DeviceId};
use pulseseek_domain::error::{ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory};

use crate::audio_device_service::{AudioDeviceService, DeviceInfoData};

/// Tauri-facing audio-device service backed by the native cpal adapter.
pub struct NativeAudioDeviceService {
    output: Arc<Mutex<CpalAudioOutput>>,
}

impl NativeAudioDeviceService {
    pub fn new(output: Arc<Mutex<CpalAudioOutput>>) -> Self {
        Self { output }
    }

    fn unavailable(message: &str) -> ApplicationError {
        ApplicationError::new(
            ErrorCategory::Unavailable,
            DiagnosticContext::new(DiagnosticCode::AudioOutput),
            std::io::Error::other(message),
        )
    }
}

impl AudioDeviceService for NativeAudioDeviceService {
    fn list_devices(&self) -> Result<Vec<DeviceInfoData>, ApplicationError> {
        let output = self.output.lock().map_err(|_| Self::unavailable("output lock poisoned"))?;
        output
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
        let output = self.output.lock().map_err(|_| Self::unavailable("output lock poisoned"))?;
        let Some(device_id) = output.current_device() else {
            return Ok(None);
        };
        let id_match = device_id.as_str().to_string();
        let devices = output.list_devices().map_err(|e| Self::unavailable(&e.to_string()))?;
        Ok(devices
            .into_iter()
            .find(|d| d.id.as_str() == id_match)
            .map(|d| DeviceInfoData { id: d.id.as_str().to_string(), name: d.name }))
    }

    fn select_device(&mut self, device_id: &str) -> Result<(), ApplicationError> {
        let mut output =
            self.output.lock().map_err(|_| Self::unavailable("output lock poisoned"))?;
        output
            .open(&DeviceId::new(device_id))
            .map_err(|error| Self::unavailable(&error.to_string()))
    }

    fn is_device_lost(&self) -> bool {
        self.output.lock().map(|output| output.is_device_lost()).unwrap_or(false)
    }
}
