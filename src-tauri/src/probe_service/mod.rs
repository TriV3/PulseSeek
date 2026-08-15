use std::path::PathBuf;

use pulseseek_browser_fs::probe::NativeProbe;
use pulseseek_domain::browser::probe::{ProbeError, ProbeFile, ProbeResult};
use pulseseek_domain::error::{ApplicationError, ErrorContract};

/// Serializable classification returned to the frontend for a dropped path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeKind {
    Directory,
    Playable,
    Unsupported,
    Missing,
}

impl From<ProbeResult> for ProbeKind {
    fn from(result: ProbeResult) -> Self {
        match result {
            ProbeResult::Directory => ProbeKind::Directory,
            ProbeResult::Playable => ProbeKind::Playable,
            ProbeResult::Unsupported => ProbeKind::Unsupported,
            ProbeResult::Missing => ProbeKind::Missing,
        }
    }
}

pub trait ProbeService: Send {
    fn probe(&self, path: String) -> Result<ProbeKind, ApplicationError>;
}

/// Concrete native probe service wired to the filesystem adapter.
pub struct GenericNativeProbeService<T: ProbeFile> {
    inner: T,
}

impl<T: ProbeFile> GenericNativeProbeService<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
}

impl<T: ProbeFile> ProbeService for GenericNativeProbeService<T> {
    fn probe(&self, path: String) -> Result<ProbeKind, ApplicationError> {
        let result = self.inner.probe(&PathBuf::from(path)).map_err(map_probe_error)?;
        Ok(ProbeKind::from(result))
    }
}

/// The concrete native probe service used by the application.
pub type NativeProbeService = GenericNativeProbeService<NativeProbe>;

pub fn native_probe_service() -> NativeProbeService {
    GenericNativeProbeService::new(NativeProbe)
}

fn map_probe_error(error: ProbeError) -> ApplicationError {
    let category = error.category();
    let context = error.diagnostic_context();
    ApplicationError::new(category, context, error)
}

pub struct FakeProbeService {
    probe: Box<dyn Fn(String) -> Result<ProbeKind, ApplicationError> + Send>,
}

impl FakeProbeService {
    pub fn new(probe: Box<dyn Fn(String) -> Result<ProbeKind, ApplicationError> + Send>) -> Self {
        Self { probe }
    }
}

impl ProbeService for FakeProbeService {
    fn probe(&self, path: String) -> Result<ProbeKind, ApplicationError> {
        (self.probe)(path)
    }
}

#[cfg(test)]
mod tests;
