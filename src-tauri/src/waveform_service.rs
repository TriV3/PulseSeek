use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::UNIX_EPOCH;

use pulseseek_cache::waveform_cache::{
    waveform_cache_key, WaveformCachePort, WaveformIdentity, WAVEFORM_FORMAT_VERSION,
};
use pulseseek_decoder_symphonia::registry::DecoderRegistry;
use pulseseek_domain::decoder::{DecodeError, Decoder};
use pulseseek_domain::error::{ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory};
use pulseseek_domain::waveform::extraction::{
    extract_overview, extract_sampled_overview, ExtractionError, ExtractionOptions,
};
use pulseseek_domain::waveform::levels::MultiresolutionWaveform;

use crate::playback_events::{PlaybackEventEmitter, WaveformReadyPayload, EVENT_WAVEFORM_READY};

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

/// Cache-first waveform service with progressive on-demand extraction.
///
/// The service returns a bounded sampled overview immediately on a cache miss,
/// then builds the exact multiresolution pyramid on a cancellable cache worker.
/// Neither path runs on an audio callback. A missing or failing cache degrades
/// to sampled rendering so the Audio Player never depends on the cache.
pub struct NativeWaveformService {
    cache: Option<Arc<dyn WaveformCachePort>>,
    open_decoder: DecoderOpener,
    active_extraction: Arc<Mutex<Option<ActiveExtraction>>>,
    next_extraction_id: AtomicU64,
    events: Option<Arc<dyn PlaybackEventEmitter>>,
}

struct ActiveExtraction {
    id: u64,
    cache_key: String,
    cancelled: Arc<AtomicBool>,
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
        Self {
            cache,
            open_decoder,
            active_extraction: Arc::new(Mutex::new(None)),
            next_extraction_id: AtomicU64::new(1),
            events: None,
        }
    }

    /// Emits an event after the exact pyramid has been stored successfully.
    pub fn with_events(mut self, events: Arc<dyn PlaybackEventEmitter>) -> Self {
        self.events = Some(events);
        self
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

    fn extract_fast_preview(
        &self,
        path: &Path,
        target_peaks: u64,
    ) -> Result<MultiresolutionWaveform, ApplicationError> {
        let mut decoder = (self.open_decoder)(path).map_err(from_decode_error)?;
        extract_sampled_overview(&mut *decoder, target_peaks, &|| false)
            .map_err(from_extraction_error)
    }

    fn start_background_extraction(&self, path: PathBuf, identity: WaveformIdentity) {
        let Some(cache) = self.cache.clone() else {
            return;
        };
        let cache_key = waveform_cache_key(&identity);
        let Some((id, cancelled)) = self.begin_extraction(cache_key.clone()) else {
            return;
        };
        let open_decoder = Arc::clone(&self.open_decoder);
        let active_extraction = Arc::clone(&self.active_extraction);
        let cancelled_for_worker = Arc::clone(&cancelled);
        let events = self.events.clone();

        let worker = thread::Builder::new()
            .name("pulseseek-waveform-cache".to_string())
            .spawn(move || {
                let result = open_decoder(&path)
                    .map_err(ExtractionError::Decode)
                    .and_then(|mut decoder| {
                        extract_overview(
                            &mut *decoder,
                            &ExtractionOptions::default_overview(),
                            &|| cancelled_for_worker.load(Ordering::Acquire),
                        )
                    });
                match result {
                    Ok(waveform) if !cancelled_for_worker.load(Ordering::Acquire) => {
                        match cache.store_waveform(&cache_key, &identity, &waveform) {
                            Ok(()) => {
                                if let Some(events) = events {
                                    let payload = WaveformReadyPayload {
                                        path: path.to_string_lossy().to_string(),
                                    };
                                    if events
                                        .emit(
                                            EVENT_WAVEFORM_READY,
                                            serde_json::to_value(payload)
                                                .expect("waveform ready payload"),
                                        )
                                        .is_err()
                                    {
                                        tracing::debug!(
                                            "waveform ready event could not be delivered"
                                        );
                                    }
                                }
                            },
                            Err(error) => {
                                tracing::warn!(error = %error, "waveform cache write failed; continuing");
                            },
                        }
                    },
                    Ok(_) | Err(ExtractionError::Cancelled) => {},
                    Err(error) => {
                        tracing::warn!(error = %error, "background waveform extraction failed");
                    },
                }
                Self::finish_extraction_state(&active_extraction, id);
            });

        if let Err(error) = worker {
            cancelled.store(true, Ordering::Release);
            self.finish_extraction(id);
            tracing::warn!(error = %error, "waveform cache worker could not start");
        }
    }

    fn begin_extraction(&self, cache_key: String) -> Option<(u64, Arc<AtomicBool>)> {
        let id = self.next_extraction_id.fetch_add(1, Ordering::Relaxed);
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut active = self.active_extraction.lock().unwrap_or_else(|error| error.into_inner());
        if active.as_ref().is_some_and(|extraction| extraction.cache_key == cache_key) {
            return None;
        }
        if let Some(previous) =
            active.replace(ActiveExtraction { id, cache_key, cancelled: Arc::clone(&cancelled) })
        {
            previous.cancelled.store(true, Ordering::Release);
        }
        Some((id, cancelled))
    }

    fn cancel_active_extraction_for_other(&self, cache_key: &str) {
        let mut active = self.active_extraction.lock().unwrap_or_else(|error| error.into_inner());
        if active.as_ref().is_some_and(|extraction| extraction.cache_key == cache_key) {
            return;
        }
        if let Some(previous) = active.take() {
            previous.cancelled.store(true, Ordering::Release);
        }
    }

    fn finish_extraction(&self, id: u64) {
        Self::finish_extraction_state(&self.active_extraction, id);
    }

    fn finish_extraction_state(active_extraction: &Mutex<Option<ActiveExtraction>>, id: u64) {
        let mut active = active_extraction.lock().unwrap_or_else(|error| error.into_inner());
        if active.as_ref().is_some_and(|extraction| extraction.id == id) {
            active.take();
        }
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

        let cache_key = waveform_cache_key(&identity);
        self.cancel_active_extraction_for_other(&cache_key);

        if let Some(waveform) = self.load_cached(&identity) {
            return Ok(level_from_waveform(&waveform, request.target_peaks));
        }

        let preview = self.extract_fast_preview(&request.path, request.target_peaks)?;
        let level = level_from_waveform(&preview, request.target_peaks);
        self.start_background_extraction(request.path.clone(), identity);
        Ok(level)
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
    use std::thread;
    use std::time::Duration as StdDuration;
    use std::time::UNIX_EPOCH;

    use pulseseek_cache::waveform_cache::{
        waveform_cache_key, WaveformCacheError, WaveformCachePort, WaveformIdentity,
    };
    use pulseseek_domain::decoder::{DecodeError, Decoder, ProbeResult, StreamMetadata};
    use pulseseek_domain::error::{
        DiagnosticCode, DiagnosticContext, ErrorCategory, ErrorContract,
    };
    use pulseseek_domain::playback::position::{Duration, Position, SeekTarget};
    use pulseseek_domain::waveform::extraction::{
        FAST_PREVIEW_PEAK_COUNT, MAX_SAMPLED_PREVIEW_PEAK_COUNT,
    };
    use pulseseek_domain::waveform::levels::{Level, LevelIndex, MultiresolutionWaveform};
    use pulseseek_domain::waveform::peak::Peak;

    use crate::playback_events::{FakeEventEmitter, EVENT_WAVEFORM_READY};

    use super::{DecoderOpener, NativeWaveformService, WaveformRequest, WaveformService};

    /// A fake decoder that plays back pre-recorded PCM data with seek support.
    struct FakeDecoder {
        data: Vec<f32>,
        position: usize,
        channels: u16,
        sample_rate: u32,
        duration: Duration,
        read_error: bool,
        read_counter: Option<Arc<AtomicUsize>>,
        read_delay: StdDuration,
        coarse_seek_used: bool,
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
                read_counter: None,
                read_delay: StdDuration::ZERO,
                coarse_seek_used: false,
            }
        }

        fn failing(mut self) -> Self {
            self.read_error = true;
            self
        }

        fn with_slow_reads(mut self, counter: Arc<AtomicUsize>) -> Self {
            self.read_counter = Some(counter);
            self.read_delay = StdDuration::from_millis(2);
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
            if !self.coarse_seek_used {
                if let Some(counter) = &self.read_counter {
                    counter.fetch_add(1, Ordering::SeqCst);
                    thread::sleep(self.read_delay);
                }
            }
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
            self.coarse_seek_used = false;
            let frame = target.position().as_millis() * self.sample_rate as u64 / 1000;
            self.position = (frame * self.channels as u64) as usize;
            Ok(Position::from_millis(target.position().as_millis()))
        }

        fn seek_coarse(&mut self, target: SeekTarget) -> Result<Position, DecodeError> {
            self.coarse_seek_used = true;
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

        fn delete_waveform(&self, key: &str) -> Result<(), WaveformCacheError> {
            self.entries.lock().unwrap().remove(key);
            Ok(())
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

    fn slow_opener(reads: Arc<AtomicUsize>) -> DecoderOpener {
        Arc::new(move |_path: &Path| {
            Ok(Box::new(
                FakeDecoder::new(vec![0.0; 1_000_000], 1, 1000, 1_000_000)
                    .with_slow_reads(Arc::clone(&reads)),
            ))
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

        assert!(calls.load(Ordering::SeqCst) >= 1, "preview decoder opened");
        assert_eq!(level.channels, 1);
        assert_eq!(level.samples_per_peak, 50, "sampled level spans the complete timeline");
        assert_eq!(level.min.len(), 4, "one bucket per target peak");

        let key = waveform_cache_key(&identity);
        let stored = (0..100)
            .find_map(|_| {
                let waveform = cache_arc.load_waveform(&key, &identity).expect("load");
                if waveform.is_none() {
                    thread::sleep(StdDuration::from_millis(2));
                }
                waveform
            })
            .expect("background extraction stored the exact pyramid");
        assert!(!stored.is_empty(), "pyramid stored in cache");
    }

    #[test]
    fn background_extraction_emits_ready_after_storing_exact_waveform() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_source(&dir, b"pcm");
        let cache: Arc<dyn WaveformCachePort> = Arc::new(FakeWaveformCache::new());
        let events = Arc::new(FakeEventEmitter::new());
        let events_port: Arc<dyn crate::playback_events::PlaybackEventEmitter> = events.clone();
        let (opener, _) = counting_opener(vec![0.25; 400], 1, 1000, 400);
        let service = NativeWaveformService::with_decoder_opener(Some(cache), opener)
            .with_events(events_port);

        service
            .get_level(&WaveformRequest { path: path.clone(), target_peaks: 256 })
            .expect("sampled preview succeeds");

        let event = (0..100)
            .find_map(|_| {
                let event = events
                    .recorded_events()
                    .into_iter()
                    .find(|event| event.event == EVENT_WAVEFORM_READY);
                if event.is_none() {
                    thread::sleep(StdDuration::from_millis(2));
                }
                event
            })
            .expect("ready event emitted after exact waveform is stored");
        assert_eq!(event.payload["path"], path.to_string_lossy().as_ref());
    }

    #[test]
    fn fast_preview_starts_exact_extraction_without_waiting_for_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_source(&dir, b"pcm");
        let identity = identity_for(&path);
        let cache: Arc<dyn WaveformCachePort> = Arc::new(FakeWaveformCache::new());
        let (opener, calls) = counting_opener(vec![0.0; 2_000], 1, 1000, 2_000);
        let service = NativeWaveformService::with_decoder_opener(Some(Arc::clone(&cache)), opener);

        let level = service
            .get_level(&WaveformRequest { path, target_peaks: FAST_PREVIEW_PEAK_COUNT })
            .expect("fast preview");

        assert_eq!(level.min.len(), FAST_PREVIEW_PEAK_COUNT as usize);
        let key = waveform_cache_key(&identity);
        let stored = (0..100)
            .find_map(|_| {
                let waveform = cache.load_waveform(&key, &identity).expect("cache read");
                if waveform.is_none() {
                    thread::sleep(StdDuration::from_millis(2));
                }
                waveform
            })
            .expect("fast preview starts the exact cache extraction");
        assert!(!stored.is_empty());
        assert!(calls.load(Ordering::SeqCst) >= 2, "preview and exact decoders opened");
    }

    #[test]
    fn new_preview_cancels_older_and_starts_its_exact_extraction() {
        let dir = tempfile::tempdir().unwrap();
        let detailed_path = temp_source(&dir, b"pcm");
        let detailed_identity = identity_for(&detailed_path);
        let preview_path = dir.path().join("next.wav");
        std::fs::write(&preview_path, b"pcm").expect("write next source");
        let preview_identity = identity_for(&preview_path);
        let reads = Arc::new(AtomicUsize::new(0));
        let cache: Arc<dyn WaveformCachePort> = Arc::new(FakeWaveformCache::new());
        let service = Arc::new(NativeWaveformService::with_decoder_opener(
            Some(Arc::clone(&cache)),
            slow_opener(Arc::clone(&reads)),
        ));
        let detailed = service
            .get_level(&WaveformRequest { path: detailed_path, target_peaks: 4_096 })
            .expect("sampled detail succeeds without waiting for the exact scan");
        assert_eq!(detailed.min.len(), MAX_SAMPLED_PREVIEW_PEAK_COUNT as usize);
        while reads.load(Ordering::SeqCst) == 0 {
            thread::yield_now();
        }

        service
            .get_level(&WaveformRequest {
                path: preview_path,
                target_peaks: FAST_PREVIEW_PEAK_COUNT,
            })
            .expect("new preview succeeds");

        for _ in 0..300 {
            if service.active_extraction.lock().unwrap().is_none() {
                break;
            }
            thread::sleep(StdDuration::from_millis(2));
        }
        assert!(service.active_extraction.lock().unwrap().is_none());
        let key = waveform_cache_key(&detailed_identity);
        assert!(cache.load_waveform(&key, &detailed_identity).expect("cache read").is_none());
        let preview_key = waveform_cache_key(&preview_identity);
        assert!(
            cache
                .load_waveform(&preview_key, &preview_identity)
                .expect("preview cache read")
                .is_some(),
            "the newly selected file should finish its own exact extraction"
        );
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
        assert!(calls.load(Ordering::SeqCst) >= 1);
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
