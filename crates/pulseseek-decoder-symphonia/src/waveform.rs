use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use pulseseek_domain::decoder::Decoder;
use pulseseek_domain::waveform::extraction::{
    extract_overview, extract_window, ExtractionError, ExtractionOptions, WindowRequest,
};
use pulseseek_domain::waveform::levels::MultiresolutionWaveform;
use pulseseek_domain::waveform::peak::Peak;

use crate::registry::DecoderRegistry;

/// Runs waveform extraction on a dedicated cancellable thread.
///
/// Extraction never runs on the audio callback: this worker owns its own
/// thread and exposes cancellation through an atomic flag checked between
/// read batches. Dropping the worker cancels and joins it.
pub struct WaveformExtractionWorker<T> {
    cancel: Arc<AtomicBool>,
    join: Option<JoinHandle<Result<T, ExtractionError>>>,
}

impl<T> WaveformExtractionWorker<T> {
    fn spawn(f: impl FnOnce(&AtomicBool) -> Result<T, ExtractionError> + Send + 'static) -> Self
    where
        T: Send + 'static,
    {
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&cancel);
        let join = thread::spawn(move || f(&flag));
        Self { cancel, join: Some(join) }
    }

    /// Requests cancellation. The running extraction stops at the next batch
    /// boundary and `wait` returns [`ExtractionError::Cancelled`].
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }

    /// Waits for the worker to finish and returns its result.
    pub fn wait(mut self) -> Result<T, ExtractionError> {
        let join = self.join.take().expect("worker already waited");
        join.join().expect("waveform extraction worker panicked")
    }
}

impl<T> Drop for WaveformExtractionWorker<T> {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl WaveformExtractionWorker<MultiresolutionWaveform> {
    /// Starts an overview extraction for an already-opened decoder.
    pub fn start_overview(decoder: Box<dyn Decoder>, options: ExtractionOptions) -> Self {
        Self::spawn(move |flag| {
            let mut decoder = decoder;
            extract_overview(decoder.as_mut(), &options, &|| flag.load(Ordering::Acquire))
        })
    }

    /// Opens `path` through the decoder registry and starts an overview
    /// extraction. Decoder selection is based on content, not extension.
    pub fn start_overview_from_path(
        path: impl AsRef<Path>,
        options: ExtractionOptions,
    ) -> Result<Self, ExtractionError> {
        let decoder = DecoderRegistry::open(path).map_err(ExtractionError::Decode)?;
        Ok(Self::start_overview(decoder, options))
    }
}

impl WaveformExtractionWorker<Vec<Peak>> {
    /// Starts a focused window extraction for an already-opened decoder.
    pub fn start_window(decoder: Box<dyn Decoder>, request: WindowRequest) -> Self {
        Self::spawn(move |flag| {
            let mut decoder = decoder;
            extract_window(decoder.as_mut(), &request, &|| flag.load(Ordering::Acquire))
        })
    }

    /// Opens `path` through the decoder registry and starts a window
    /// extraction.
    pub fn start_window_from_path(
        path: impl AsRef<Path>,
        request: WindowRequest,
    ) -> Result<Self, ExtractionError> {
        let decoder = DecoderRegistry::open(path).map_err(ExtractionError::Decode)?;
        Ok(Self::start_window(decoder, request))
    }
}
