use std::error::Error;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use pulseseek_domain::visualization::{
    SpectrumFrame, VisualizationFrameError, MAX_VISUALIZATION_FRAME_SAMPLES,
};
use pulseseek_playback::{
    visualization_channel, FftError, FftWorker, SpectrumReceiver, VisualizationTap,
};

use crate::playback_events::types::{SpectrumFramePayload, SPECTRUM_FORMAT_VERSION};
use crate::playback_events::{PlaybackEventEmitter, EVENT_SPECTRUM_FRAME};

const INPUT_CAPACITY: usize = 4;
const OUTPUT_CAPACITY: usize = 2;
const REPORTER_POLL_INTERVAL: Duration = Duration::from_millis(20);
const TARGET_SPECTRUM_FPS: u32 = 30;

/// Owns the off-callback FFT and event reporter threads for one playback stream.
pub(crate) struct VisualizationPipeline {
    stop: Arc<AtomicBool>,
    reporter: Option<JoinHandle<()>>,
    fft_worker: Option<FftWorker>,
}

impl VisualizationPipeline {
    pub(crate) fn start(
        sample_rate: u32,
        channels: usize,
        events: Arc<dyn PlaybackEventEmitter>,
    ) -> Result<(Self, VisualizationTap), VisualizationPipelineError> {
        let fft_size = fft_size_for_channels(channels)
            .ok_or(VisualizationPipelineError::UnsupportedChannels(channels))?;
        let channel_count = u16::try_from(channels)
            .map_err(|_| VisualizationPipelineError::UnsupportedChannels(channels))?;
        let sample_count = fft_size
            .checked_mul(channels)
            .ok_or(VisualizationPipelineError::UnsupportedChannels(channels))?;
        let (publisher, subscriber) = visualization_channel(INPUT_CAPACITY);
        let hop_frames = spectrum_hop_frames(sample_rate, fft_size);
        let tap = VisualizationTap::new_with_hop_frames(
            publisher,
            sample_rate,
            channel_count,
            sample_count,
            hop_frames,
        )
        .map_err(VisualizationPipelineError::InvalidTap)?;
        let (fft_worker, spectra) = FftWorker::start(subscriber, fft_size, OUTPUT_CAPACITY)
            .map_err(VisualizationPipelineError::Fft)?;
        let stop = Arc::new(AtomicBool::new(false));
        let reporter_stop = Arc::clone(&stop);
        let reporter = thread::Builder::new()
            .name("pulseseek-spectrum-events".to_string())
            .spawn(move || report_spectra(spectra, reporter_stop, events))
            .map_err(|_| VisualizationPipelineError::ReporterStartFailed)?;
        Ok((Self { stop, reporter: Some(reporter), fft_worker: Some(fft_worker) }, tap))
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(reporter) = self.reporter.take() {
            let _ = reporter.join();
        }
        self.fft_worker = None;
    }
}

impl Drop for VisualizationPipeline {
    fn drop(&mut self) {
        self.stop();
    }
}

fn fft_size_for_channels(channels: usize) -> Option<usize> {
    if channels == 0 {
        return None;
    }
    let available_frames = MAX_VISUALIZATION_FRAME_SAMPLES.checked_div(channels)?;
    if available_frames < 2 {
        return None;
    }
    Some(1_usize << available_frames.ilog2())
}

fn spectrum_hop_frames(sample_rate: u32, fft_size: usize) -> usize {
    usize::try_from(sample_rate / TARGET_SPECTRUM_FPS).unwrap_or(fft_size).clamp(1, fft_size)
}

fn report_spectra(
    spectra: SpectrumReceiver,
    stop: Arc<AtomicBool>,
    events: Arc<dyn PlaybackEventEmitter>,
) {
    while !stop.load(Ordering::Acquire) {
        let mut frame = match spectra.recv_timeout(REPORTER_POLL_INTERVAL) {
            Ok(frame) => frame,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        if let Some(latest) = spectra.try_receive_latest() {
            frame = latest.frame;
        }
        if !events.try_begin_spectrum_delivery() {
            continue;
        }
        let payload = spectrum_payload(&frame);
        let Ok(value) = serde_json::to_value(payload) else {
            events.acknowledge_spectrum();
            continue;
        };
        if events.emit(EVENT_SPECTRUM_FRAME, value).is_err() {
            events.acknowledge_spectrum();
        }
    }
}

fn spectrum_payload(frame: &SpectrumFrame) -> SpectrumFramePayload {
    SpectrumFramePayload {
        format_version: SPECTRUM_FORMAT_VERSION,
        sequence: frame.sequence(),
        position_frames: frame.position_frames(),
        sample_rate: frame.sample_rate(),
        fft_size: frame.fft_size(),
        magnitudes: frame.magnitudes().to_vec(),
    }
}

#[derive(Debug)]
pub(crate) enum VisualizationPipelineError {
    UnsupportedChannels(usize),
    InvalidTap(VisualizationFrameError),
    Fft(FftError),
    ReporterStartFailed,
}

impl fmt::Display for VisualizationPipelineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedChannels(channels) => {
                write!(formatter, "unsupported visualization channel count: {channels}")
            },
            Self::InvalidTap(error) => write!(formatter, "invalid visualization tap: {error}"),
            Self::Fft(error) => write!(formatter, "FFT worker unavailable: {error}"),
            Self::ReporterStartFailed => formatter.write_str("spectrum reporter could not start"),
        }
    }
}

impl Error for VisualizationPipelineError {}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use pulseseek_domain::visualization::SpectrumFrame;

    use crate::playback_events::FakeEventEmitter;

    use super::*;

    #[test]
    fn spectrum_payload_preserves_frame_metadata() {
        let frame = SpectrumFrame::new(9, 4_096, 48_000, 8, vec![0.0, 0.1, 0.2, 0.3, 0.4])
            .expect("valid spectrum");

        let payload = spectrum_payload(&frame);

        assert_eq!(payload.format_version, 1);
        assert_eq!(payload.sequence, 9);
        assert_eq!(payload.position_frames, 4_096);
        assert_eq!(payload.sample_rate, 48_000);
        assert_eq!(payload.fft_size, 8);
        assert_eq!(payload.magnitudes, frame.magnitudes());
    }

    #[test]
    fn pipeline_starts_and_stops_for_stereo_output() {
        let events = Arc::new(FakeEventEmitter::new());

        let (pipeline, _tap) =
            VisualizationPipeline::start(48_000, 2, events).expect("stereo visualization pipeline");

        drop(pipeline);
    }

    #[test]
    fn pipeline_rejects_channel_layout_larger_than_fixed_frame_storage() {
        let events = Arc::new(FakeEventEmitter::new());

        let result = VisualizationPipeline::start(48_000, 8_193, events);

        assert!(matches!(result, Err(VisualizationPipelineError::UnsupportedChannels(8_193))));
    }

    #[test]
    fn stereo_pipeline_uses_enough_bins_for_low_frequency_detail() {
        assert_eq!(fft_size_for_channels(2), Some(4_096));
    }

    #[test]
    fn spectrum_hop_targets_thirty_updates_per_second() {
        assert_eq!(spectrum_hop_frames(48_000, 4_096), 1_600);
        assert_eq!(spectrum_hop_frames(44_100, 4_096), 1_470);
    }
}
