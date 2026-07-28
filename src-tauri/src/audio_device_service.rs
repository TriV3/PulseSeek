use serde::{Deserialize, Serialize};

use pulseseek_domain::error::{ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory};

/// Serializable device information sent to the frontend.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceInfoData {
    pub id: String,
    pub name: String,
}

/// Application service for querying and selecting audio output devices.
///
/// This trait abstracts the domain [`AudioOutput`] port behind a narrow
/// command interface. No concrete adapter is exposed across the Tauri
/// boundary.
pub trait AudioDeviceService: Send {
    /// Returns all available output devices.
    fn list_devices(&self) -> Result<Vec<DeviceInfoData>, ApplicationError>;

    /// Returns the currently selected device, if any.
    fn current_device(&self) -> Result<Option<DeviceInfoData>, ApplicationError>;

    /// Selects an output device by its stable identifier.
    fn select_device(&mut self, device_id: &str) -> Result<(), ApplicationError>;

    /// Returns `true` when the active device has been lost.
    fn is_device_lost(&self) -> bool;
}

/// Fake implementation of [`AudioDeviceService`] for use in command-envelope tests.
pub struct FakeAudioDeviceService {
    pub devices: Vec<DeviceInfoData>,
    pub current: Option<DeviceInfoData>,
    pub device_lost: bool,
    pub fail_list: bool,
    pub fail_current: bool,
    pub fail_select: bool,
    pub select_call_count: u64,
    pub last_select_id: Option<String>,
}

impl FakeAudioDeviceService {
    pub fn new() -> Self {
        Self {
            devices: vec![
                DeviceInfoData { id: "default".to_string(), name: "Default Output".to_string() },
                DeviceInfoData { id: "hdmi".to_string(), name: "HDMI Output".to_string() },
            ],
            current: Some(DeviceInfoData {
                id: "default".to_string(),
                name: "Default Output".to_string(),
            }),
            device_lost: false,
            fail_list: false,
            fail_current: false,
            fail_select: false,
            select_call_count: 0,
            last_select_id: None,
        }
    }

    fn make_error() -> ApplicationError {
        ApplicationError::new(
            ErrorCategory::Unavailable,
            DiagnosticContext::new(DiagnosticCode::AudioOutput),
            std::io::Error::other("fake device error"),
        )
    }
}

impl Default for FakeAudioDeviceService {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioDeviceService for FakeAudioDeviceService {
    fn list_devices(&self) -> Result<Vec<DeviceInfoData>, ApplicationError> {
        if self.fail_list {
            return Err(Self::make_error());
        }
        Ok(self.devices.clone())
    }

    fn current_device(&self) -> Result<Option<DeviceInfoData>, ApplicationError> {
        if self.fail_current {
            return Err(Self::make_error());
        }
        Ok(self.current.clone())
    }

    fn select_device(&mut self, device_id: &str) -> Result<(), ApplicationError> {
        self.select_call_count += 1;
        self.last_select_id = Some(device_id.to_string());
        if self.fail_select {
            return Err(Self::make_error());
        }
        // Update current device if found in list.
        if let Some(device) = self.devices.iter().find(|d| d.id == device_id).cloned() {
            self.current = Some(device);
        }
        Ok(())
    }

    fn is_device_lost(&self) -> bool {
        self.device_lost
    }
}
