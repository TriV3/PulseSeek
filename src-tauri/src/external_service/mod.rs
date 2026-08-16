use std::path::Path;

use pulseseek_domain::browser::external_actions::{ExternalActionError, ExternalActions};
use pulseseek_domain::error::{ApplicationError, ErrorContract};

pub trait ExternalService: Send {
    fn reveal(&self, path: String) -> Result<(), ApplicationError>;
    fn open_with(&self, path: String) -> Result<(), ApplicationError>;
}

pub struct NativeExternalService<T: ExternalActions> {
    inner: T,
}

impl<T: ExternalActions> NativeExternalService<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
}

impl<T: ExternalActions> ExternalService for NativeExternalService<T> {
    fn reveal(&self, path: String) -> Result<(), ApplicationError> {
        self.inner.reveal(Path::new(&path)).map_err(map_external_error)
    }

    fn open_with(&self, path: String) -> Result<(), ApplicationError> {
        self.inner.open_with(Path::new(&path)).map_err(map_external_error)
    }
}

fn map_external_error(error: ExternalActionError) -> ApplicationError {
    let category = error.category();
    let context = error.diagnostic_context();
    ApplicationError::new(category, context, error)
}

pub struct FakeExternalService {
    reveal: Box<dyn Fn(String) -> Result<(), ApplicationError> + Send>,
    open: Box<dyn Fn(String) -> Result<(), ApplicationError> + Send>,
}

impl FakeExternalService {
    pub fn new(
        reveal: Box<dyn Fn(String) -> Result<(), ApplicationError> + Send>,
        open: Box<dyn Fn(String) -> Result<(), ApplicationError> + Send>,
    ) -> Self {
        Self { reveal, open }
    }
}

impl ExternalService for FakeExternalService {
    fn reveal(&self, path: String) -> Result<(), ApplicationError> {
        (self.reveal)(path)
    }

    fn open_with(&self, path: String) -> Result<(), ApplicationError> {
        (self.open)(path)
    }
}

#[cfg(test)]
mod tests;
