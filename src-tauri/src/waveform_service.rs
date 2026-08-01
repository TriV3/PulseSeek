use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use pulseseek_cache::waveform_cache::{
    waveform_cache_key, WaveformCachePort, WaveformIdentity, WAVEFORM_FORMAT_VERSION,
};
use pulseseek_decoder_symphonia::registry::DecoderRegistry;
use pulseseek_decoder_symphonia::waveform::WaveformExtractionWorker;
use pulseseek_domain::decoder::{DecodeError, Decoder};
use pulseseek_domain::error::{ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory};
use pulseseek_domain::waveform::extraction::{ExtractionError, ExtractionOptions};
use pulseseek_domain::waveform::levels::MultiresolutionWaveform;

/// One resolution level of waveform data, ready for the renderer.
///
/// The peak arrays follow the domain level layout: interleaved by bucket then
/// channel (`[ch0, ch1, ch0, ch1, ...]`), one `min`/`max` pair per peak.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct WaveformLevel {
    /// Codec/format version of the waveform payload.
    pub format_version: u32,
    /// Number of interleaved audio channels.
    pub channels: u16,
    /// Samples covered by one peak bucket at this level.
    pub samples_per_peak: u64,
    /// Lower envelope bound per peak.
    pub min: Vec<f32>,
    /// Upper envelope bound per peak.
    pub max: Vec<f32>,
}

/// Validated request for a waveform level.
#[derive(Clone, Debug, PartialEq)]
pub struct WaveformRequest {
    /// Path of the source audio file.
    pub path: PathBuf,
    /// Desired number of buckets per channel at the returned level.
    pub target_peaks: u64,
}

/// Opens a decoder for a source file.
pub type DecoderOpener = Arc<dyn Fn(&Path) -> Result<Box<dyn Decoder>, DecodeError> + Send + Sync>;

/// Port for producing waveform levels.
pub trait WaveformService: Send + Sync {
    /// Returns the resolution level that best fits `request.target_peaks`.
    fn get_level(&self, request: &WaveformRequest) -> Result<WaveformLevel, ApplicationError>;
}

/// Cache-first waveform service with on-demand extraction.
///
/// The service checks the technical cache with a versioned file identity, then
/// extracts a multiresolution overview when the cache misses. Extraction runs
/// on the extraction worker thread, never on an audio callback. A missing or
/// failing cache degrades to extraction without storage so the Audio Player
/// never depends on the cache.
pub struct NativeWaveformService {
    cache: Option<Arc<dyn WaveformCachePort>>,
    open_decoder: DecoderOpener,
}

impl NativeWaveformService {
    /// Creates a service using the real decoder registry.
    pub fn new(cache: Option<Arc<dyn WaveformCachePort>>) -> Self {
        Self::with_decoder_opener(cache, default_decoder_opener())
    }

    /// Creates a service with an injectable decoder opener for tests.
    pub fn with_decoder_opener(
        cache: Option<Arc<dyn WaveformCachePort>>,
        open_decoder: DecoderOpener,
    ) -> Self {
        Self { cache, open_decoder }
    }

    fn load_cached(&self, identity: &WaveformIdentity) -> Option<MultiresolutionWaveform> {
        let cache = self.cache.as_ref()?;
        let key = waveform_cache_key(identity);
        match cache.load_waveform(&key, identity) {
            Ok(Some(waveform)) => Some(waveform),
            Ok(None) => None,
            Err(error) => {
                tracing::warn!(error = %error, "waveform cache read failed; extracting");
                None
            },
        }
    }

    fn extract(&self, path: &Path) -> Result<MultiresolutionWaveform, ApplicationError> {
        let decoder = (self.open_decoder)(path).map_err(from_decode_error)?;
        let worker = WaveformExtractionWorker::start_overview(
            decoder,
            ExtractionOptions::default_overview(),
        );
        worker.wait().map_err(from_extraction_error)
    }
}

impl WaveformService for NativeWaveformService {
    fn get_level(&self, request: &WaveformRequest) -> Result<WaveformLevel, ApplicationError> {
        if request.target_peaks == 0 {
            return Err(ApplicationError::new(
                ErrorCategory::InvalidInput,
                DiagnosticContext::new(DiagnosticCode::BrowserRead),
                std::io::Error::other("target peaks must be positive"),
            ));
        }
        if !request.path.is_file() {
            return Err(ApplicationError::new(
                ErrorCategory::InvalidInput,
                DiagnosticContext::new(DiagnosticCode::BrowserRead),
                std::io::Error::new(std::io::ErrorKind::NotFound, "file does not exist"),
            ));
        }
        let metadata = std::fs::metadata(&request.path).map_err(|source| {
            ApplicationError::new(
                ErrorCategory::PermissionDenied,
                DiagnosticContext::new(DiagnosticCode::BrowserRead),
                source,
            )
        })?;
        let identity = WaveformIdentity::new(
            &request.path,
            metadata.len(),
            mtime_ms(&metadata).map_err(|source| {
                ApplicationError::new(
                    ErrorCategory::Unavailable,
                    DiagnosticContext::new(DiagnosticCode::BrowserRead),
                    source,
                )
            })?,
        );

        let waveform = match self.load_cached(&identity) {
            Some(waveform) => waveform,
            None => {
                let waveform = self.extract(&request.path)?;
                if let Some(cache) = &self.cache {
                    if let Err(error) =
                        cache.store_waveform(&waveform_cache_key(&identity), &identity, &waveform)
                    {
                        tracing::warn!(error = %error, "waveform cache write failed; continuing");
                    }
                }
                waveform
            },
        };

        Ok(level_from_waveform(&waveform, request.target_peaks))
    }
}

fn level_from_waveform(waveform: &MultiresolutionWaveform, target_peaks: u64) -> WaveformLevel {
    let level = waveform.select_level(target_peaks);
    WaveformLevel {
        format_version: WAVEFORM_FORMAT_VERSION,
        channels: waveform.channels(),
        samples_per_peak: level.samples_per_peak,
        min: level.peaks.iter().map(|peak| peak.min()).collect(),
        max: level.peaks.iter().map(|peak| peak.max()).collect(),
    }
}

fn mtime_ms(metadata: &std::fs::Metadata) -> Result<u64, std::io::Error> {
    let modified = metadata.modified()?;
    Ok(modified.duration_since(UNIX_EPOCH).map(|duration| duration.as_millis() as u64).unwrap_or(0))
}

fn default_decoder_opener() -> DecoderOpener {
    Arc::new(|path: &Path| DecoderRegistry::open(path))
}

fn from_decode_error(error: DecodeError) -> ApplicationError {
    ApplicationError::new(
        ErrorCategory::Unavailable,
        DiagnosticContext::new(DiagnosticCode::BrowserRead),
        error,
    )
}

fn from_extraction_error(error: ExtractionError) -> ApplicationError {
    let category = match error {
        ExtractionError::Cancelled => ErrorCategory::Cancelled,
        ExtractionError::UnsupportedSource => ErrorCategory::Unsupported,
        ExtractionError::EmptySource => ErrorCategory::InvalidInput,
        ExtractionError::Decode(_) => ErrorCategory::Unavailable,
    };
    ApplicationError::new(category, DiagnosticContext::new(DiagnosticCode::BrowserRead), error)
}

impl fmt::Debug for NativeWaveformService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeWaveformService").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::UNIX_EPOCH;

    use pulseseek_cache::waveform_cache::{
        waveform_cache_key, WaveformCacheError, WaveformCachePort, WaveformIdentity,
    };
    use pulseseek_domain::decoder::{DecodeError, Decoder, ProbeResult, StreamMetadata};
    use pulseseek_domain::error::{
        DiagnosticCode, DiagnosticContext, ErrorCategory, ErrorContract,
    };
    use pulseseek_domain::playback::position::{Duration, Position, SeekTarget};
    use pulseseek_domain::waveform::levels::{Level, LevelIndex, MultiresolutionWaveform};
    use pulseseek_domain::waveform::peak::Peak;

    use super::{DecoderOpener, NativeWaveformService, WaveformRequest, WaveformService};

    /// A fake decoder that plays back pre-recorded PCM data with seek support.
    struct FakeDecoder {
        data: Vec<f32>,
        position: usize,
        channels: u16,
        sample_rate: u32,
        duration: Duration,
        read_error: bool,
    }

    impl FakeDecoder {
        fn new(data: Vec<f32>, channels: u16, sample_rate: u32, duration_ms: u64) -> Self {
            Self {
                data,
                position: 0,
                channels,
                sample_rate,
                duration: Duration::from_millis(duration_ms),
                read_error: false,
            }
        }

        fn failing(mut self) -> Self {
            self.read_error = true;
            self
        }
    }

    impl Decoder for FakeDecoder {
        fn probe(&self) -> ProbeResult {
            ProbeResult::Supported
        }

        fn metadata(&mut self) -> Result<StreamMetadata, DecodeError> {
            Ok(StreamMetadata {
                sample_rate: self.sample_rate,
                channels: self.channels,
                duration: self.duration,
                bit_depth: None,
                codec: "test",
            })
        }

        fn read(&mut self, buf: &mut [f32]) -> Result<usize, DecodeError> {
            if self.read_error {
                return Err(DecodeError::new(
                    DiagnosticContext::new(DiagnosticCode::BrowserRead),
                    std::io::Error::other("fake corrupt stream"),
                ));
            }
            let remaining = self.data.len() - self.position;
            let to_copy = buf.len().min(remaining);
            buf[..to_copy].copy_from_slice(&self.data[self.position..self.position + to_copy]);
            self.position += to_copy;
            Ok(to_copy)
        }

        fn seek(&mut self, target: SeekTarget) -> Result<Position, DecodeError> {
            let frame = target.position().as_millis() * self.sample_rate as u64 / 1000;
            self.position = (frame * self.channels as u64) as usize;
            Ok(Position::from_millis(target.position().as_millis()))
        }
    }

    /// In-memory waveform cache port for tests.
    struct FakeWaveformCache {
        entries: Mutex<HashMap<String, MultiresolutionWaveform>>,
        fail_store: bool,
        fail_load: bool,
    }

    impl FakeWaveformCache {
        fn new() -> Self {
            Self { entries: Mutex::new(HashMap::new()), fail_store: false, fail_load: false }
        }

        fn with_entry(key: &str, waveform: MultiresolutionWaveform) -> Self {
            let cache = Self::new();
            cache.entries.lock().unwrap().insert(key.to_string(), waveform);
            cache
        }

        fn store_failing(mut self) -> Self {
            self.fail_store = true;
            self
        }

        fn load_failing(mut self) -> Self {
            self.fail_load = true;
            self
        }
    }

    impl WaveformCachePort for FakeWaveformCache {
        fn store_waveform(
            &self,
            key: &str,
            _identity: &WaveformIdentity,
            waveform: &MultiresolutionWaveform,
        ) -> Result<(), WaveformCacheError> {
            if self.fail_store {
                return Err(WaveformCacheError::WorkerStopped);
            }
            self.entries.lock().unwrap().insert(key.to_string(), waveform.clone());
            Ok(())
        }

        fn load_waveform(
            &self,
            key: &str,
            _identity: &WaveformIdentity,
        ) -> Result<Option<MultiresolutionWaveform>, WaveformCacheError> {
            if self.fail_load {
                return Err(WaveformCacheError::WorkerStopped);
            }
            Ok(self.entries.lock().unwrap().get(key).cloned())
        }
    }

    fn temp_source(dir: &tempfile::TempDir, bytes: &[u8]) -> PathBuf {
        let path = dir.path().join("source.wav");
        std::fs::write(&path, bytes).expect("write source");
        path
    }

    fn identity_for(path: &Path) -> WaveformIdentity {
        let metadata = std::fs::metadata(path).expect("metadata");
        let modified_ms = metadata
            .modified()
            .expect("modified")
            .duration_since(UNIX_EPOCH)
            .expect("epoch")
            .as_millis() as u64;
        WaveformIdentity::new(path, metadata.len(), modified_ms)
    }

    fn stereo_two_levels() -> MultiresolutionWaveform {
        let coarsest = Level {
            index: LevelIndex::new(0).expect("level 0"),
            samples_per_peak: 4,
            peaks: vec![Peak::from_parts(-0.5, 0.5), Peak::from_parts(-0.25, 0.75)],
        };
        let finest = Level {
            index: LevelIndex::new(1).expect("level 1"),
            samples_per_peak: 2,
            peaks: vec![
                Peak::from_parts(-0.4, 0.4),
                Peak::from_parts(-0.3, 0.6),
                Peak::from_parts(-0.2, 0.8),
                Peak::from_parts(-0.1, 0.9),
            ],
        };
        MultiresolutionWaveform::from_levels(2, vec![coarsest, finest]).expect("valid waveform")
    }

    /// Returns a decoder opener backed by a fresh fake decoder per call plus a
    /// call counter.
    fn counting_opener(
        data: Vec<f32>,
        channels: u16,
        sample_rate: u32,
        duration_ms: u64,
    ) -> (DecoderOpener, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&calls);
        let opener: DecoderOpener = Arc::new(move |_path: &Path| {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(FakeDecoder::new(data.clone(), channels, sample_rate, duration_ms)))
        });
        (opener, calls)
    }

    fn failing_opener() -> DecoderOpener {
        Arc::new(move |_path: &Path| {
            Ok(Box::new(FakeDecoder::new(vec![0.0; 10], 1, 1000, 10).failing()))
        })
    }

    #[test]
    fn cache_hit_returns_stored_level_without_extraction() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_source(&dir, b"pcm");
        let identity = identity_for(&path);
        let key = waveform_cache_key(&identity);
        let cache = FakeWaveformCache::with_entry(&key, stereo_two_levels());
        let (opener, calls) = counting_opener(vec![0.0; 100], 2, 1000, 50);

        let service = NativeWaveformService::with_decoder_opener(Some(Arc::new(cache)), opener);
        let level = service
            .get_level(&WaveformRequest { path: path.clone(), target_peaks: 1 })
            .expect("cache hit");

        assert_eq!(calls.load(Ordering::SeqCst), 0, "no extraction on cache hit");
        assert_eq!(level.channels, 2);
        assert_eq!(level.samples_per_peak, 4, "coarsest level satisfies target 1");
        assert_eq!(level.min, vec![-0.5, -0.25]);
        assert_eq!(level.max, vec![0.5, 0.75]);
    }

    #[test]
    fn cache_miss_extracts_stores_and_returns() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_source(&dir, b"pcm");
        let identity = identity_for(&path);
        let cache = FakeWaveformCache::new();
        let data: Vec<f32> = (0..400).map(|i| (i as f32 / 400.0) - 0.5).collect();
        let (opener, calls) = counting_opener(data, 1, 1000, 200);
        let cache_arc: Arc<dyn WaveformCachePort> = Arc::new(cache);

        let service =
            NativeWaveformService::with_decoder_opener(Some(Arc::clone(&cache_arc)), opener);
        let level = service
            .get_level(&WaveformRequest { path, target_peaks: 4 })
            .expect("extraction succeeds");

        assert_eq!(calls.load(Ordering::SeqCst), 1, "exactly one extraction");
        assert_eq!(level.channels, 1);
        assert_eq!(level.samples_per_peak, 128, "coarsest level that fits 4 buckets");
        assert_eq!(level.min.len(), 4, "one bucket per target peak");

        let key = waveform_cache_key(&identity);
        let stored = cache_arc.load_waveform(&key, &identity).expect("load").expect("stored");
        assert!(!stored.is_empty(), "pyramid stored in cache");
    }

    #[test]
    fn selects_level_satisfying_target_peaks() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_source(&dir, b"pcm");
        let identity = identity_for(&path);
        let key = waveform_cache_key(&identity);
        let cache = FakeWaveformCache::with_entry(&key, stereo_two_levels());
        let (opener, _) = counting_opener(vec![], 2, 1000, 10);

        let service = NativeWaveformService::with_decoder_opener(Some(Arc::new(cache)), opener);

        let fine = service
            .get_level(&WaveformRequest { path: path.clone(), target_peaks: 2 })
            .expect("fine level");
        assert_eq!(fine.samples_per_peak, 2, "finest level has >= 2 buckets");

        let coarse = service
            .get_level(&WaveformRequest { path: path.clone(), target_peaks: 1 })
            .expect("coarse level");
        assert_eq!(coarse.samples_per_peak, 4, "coarsest level satisfies target 1");

        let fallback = service
            .get_level(&WaveformRequest { path, target_peaks: 100 })
            .expect("fallback level");
        assert_eq!(fallback.samples_per_peak, 2, "falls back to finest beyond every level");
    }

    #[test]
    fn target_peaks_zero_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_source(&dir, b"pcm");
        let service = NativeWaveformService::with_decoder_opener(
            None,
            counting_opener(vec![], 1, 1000, 10).0,
        );

        let err = service
            .get_level(&WaveformRequest { path, target_peaks: 0 })
            .expect_err("zero target rejected");
        assert_eq!(err.user_descriptor().category(), ErrorCategory::InvalidInput);
    }

    #[test]
    fn missing_file_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing.wav");
        let service = NativeWaveformService::with_decoder_opener(
            None,
            counting_opener(vec![], 1, 1000, 10).0,
        );

        let err = service
            .get_level(&WaveformRequest { path: missing, target_peaks: 4 })
            .expect_err("missing file rejected");
        assert_eq!(err.user_descriptor().category(), ErrorCategory::InvalidInput);
    }

    #[test]
    fn extraction_failure_maps_to_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_source(&dir, b"pcm");
        let service = NativeWaveformService::with_decoder_opener(None, failing_opener());

        let err = service
            .get_level(&WaveformRequest { path, target_peaks: 4 })
            .expect_err("corrupt stream rejected");
        assert_eq!(err.user_descriptor().category(), ErrorCategory::Unavailable);
    }

    #[test]
    fn without_cache_still_extracts_and_returns() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_source(&dir, b"pcm");
        let (opener, calls) = counting_opener(vec![0.0; 100], 1, 1000, 50);

        let service = NativeWaveformService::with_decoder_opener(None, opener);
        let level = service
            .get_level(&WaveformRequest { path, target_peaks: 2 })
            .expect("extraction without cache");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(level.channels, 1);
    }

    #[test]
    fn cache_read_failure_degrades_to_extraction() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_source(&dir, b"pcm");
        let cache = FakeWaveformCache::new().load_failing();
        let (opener, calls) = counting_opener(vec![0.0; 100], 1, 1000, 50);

        let service = NativeWaveformService::with_decoder_opener(Some(Arc::new(cache)), opener);
        let level = service
            .get_level(&WaveformRequest { path, target_peaks: 2 })
            .expect("degrades to extraction");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(level.channels, 1);
    }

    #[test]
    fn cache_write_failure_does_not_fail_request() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_source(&dir, b"pcm");
        let cache = FakeWaveformCache::new().store_failing();
        let (opener, _) = counting_opener(vec![0.0; 100], 1, 1000, 50);

        let service = NativeWaveformService::with_decoder_opener(Some(Arc::new(cache)), opener);
        let level = service
            .get_level(&WaveformRequest { path, target_peaks: 2 })
            .expect("write failure is non-fatal");
        assert_eq!(level.channels, 1);
    }
}
