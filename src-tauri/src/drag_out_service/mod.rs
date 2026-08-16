use std::path::PathBuf;

use pulseseek_browser_fs::drag_out::NativeDragOut;
use pulseseek_domain::browser::drag_out::{DragOut, DragOutError};
use pulseseek_domain::error::{ApplicationError, ErrorContract};

#[cfg(target_os = "macos")]
mod native_macos;
#[cfg(target_os = "macos")]
pub use native_macos::NativeMacosDragStarter;

#[cfg(not(target_os = "macos"))]
mod native_other;
#[cfg(not(target_os = "macos"))]
pub use native_other::UnsupportedDragStarter;

/// The concrete drag starter used by the native service.
#[cfg(target_os = "macos")]
pub type NativeDragStarter = NativeMacosDragStarter;
/// The concrete drag starter used by the native service.
#[cfg(not(target_os = "macos"))]
pub type NativeDragStarter = UnsupportedDragStarter;

/// The concrete native drag-out service, wired to the platform starter.
pub type NativeDragOutService = GenericNativeDragOutService<NativeDragOut<NativeDragStarter>>;

pub trait DragOutService: Send {
    fn drag_out(&self, paths: Vec<String>) -> Result<(), ApplicationError>;
}

pub struct GenericNativeDragOutService<T: DragOut> {
    inner: T,
}

impl<T: DragOut> GenericNativeDragOutService<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
}

impl<T: DragOut> DragOutService for GenericNativeDragOutService<T> {
    fn drag_out(&self, paths: Vec<String>) -> Result<(), ApplicationError> {
        let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
        self.inner.drag_out(&paths).map_err(map_drag_out_error)
    }
}

/// Builds the native drag-out service wired to the platform drag starter.
pub fn native_drag_out_service(app: tauri::AppHandle) -> NativeDragOutService {
    GenericNativeDragOutService::new(NativeDragOut::new(native_drag_starter(app)))
}

#[cfg(target_os = "macos")]
fn native_drag_starter(app: tauri::AppHandle) -> NativeMacosDragStarter {
    NativeMacosDragStarter::new(app)
}

#[cfg(not(target_os = "macos"))]
fn native_drag_starter(_app: tauri::AppHandle) -> UnsupportedDragStarter {
    UnsupportedDragStarter
}

fn map_drag_out_error(error: DragOutError) -> ApplicationError {
    let category = error.category();
    let context = error.diagnostic_context();
    ApplicationError::new(category, context, error)
}

pub struct FakeDragOutService {
    drag: Box<dyn Fn(Vec<String>) -> Result<(), ApplicationError> + Send>,
}

impl FakeDragOutService {
    pub fn new(drag: Box<dyn Fn(Vec<String>) -> Result<(), ApplicationError> + Send>) -> Self {
        Self { drag }
    }
}

impl DragOutService for FakeDragOutService {
    fn drag_out(&self, paths: Vec<String>) -> Result<(), ApplicationError> {
        (self.drag)(paths)
    }
}

#[cfg(test)]
mod tests;
