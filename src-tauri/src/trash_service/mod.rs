use std::path::PathBuf;

use pulseseek_domain::browser::trash::{FileTrash, TrashResult};
use pulseseek_domain::error::{ApplicationError, ErrorContract};

pub trait TrashService: Send {
    fn move_to_trash(&self, paths: Vec<PathBuf>) -> Vec<(PathBuf, Result<(), ApplicationError>)>;
}

#[allow(clippy::type_complexity)]
pub struct NativeTrashService<T: FileTrash> {
    inner: T,
}

impl<T: FileTrash> NativeTrashService<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
}

impl<T: FileTrash> TrashService for NativeTrashService<T> {
    fn move_to_trash(&self, paths: Vec<PathBuf>) -> Vec<(PathBuf, Result<(), ApplicationError>)> {
        let results: Vec<TrashResult> = self.inner.move_to_trash(&paths);
        results
            .into_iter()
            .map(|(path, result)| {
                let mapped = result.map_err(|e| {
                    let category = e.category();
                    let context = e.diagnostic_context();
                    ApplicationError::new(category, context, e)
                });
                (path, mapped)
            })
            .collect()
    }
}

#[allow(clippy::type_complexity)]
pub struct FakeTrashService {
    f: Box<dyn Fn(Vec<PathBuf>) -> Vec<(PathBuf, Result<(), ApplicationError>)> + Send>,
}

#[allow(clippy::type_complexity)]
impl FakeTrashService {
    pub fn new(
        f: Box<dyn Fn(Vec<PathBuf>) -> Vec<(PathBuf, Result<(), ApplicationError>)> + Send>,
    ) -> Self {
        Self { f }
    }
}

impl TrashService for FakeTrashService {
    fn move_to_trash(&self, paths: Vec<PathBuf>) -> Vec<(PathBuf, Result<(), ApplicationError>)> {
        (self.f)(paths)
    }
}

#[cfg(test)]
mod tests;
