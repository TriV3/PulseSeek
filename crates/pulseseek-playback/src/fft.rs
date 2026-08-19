use std::error::Error;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use pulseseek_domain::analysis_subscriptions::WindowFunction;
use pulseseek_domain::visualization::{
    SpectrumFrame, SpectrumFrameError, VisualizationFrame, MAX_VISUALIZATION_FRAME_SAMPLES,
};
use realfft::{RealFftPlanner, RealToComplex};

use crate::{VisualizationControl, VisualizationSubscriber};

pub const SUPPORTED_FFT_SIZES: [usize; 4] = [2_048, 4_096, 8_192, 16_384];
pub const SUPPORTED_WINDOWS: [WindowFunction; 5] = [
    WindowFunction::Hann,
    WindowFunction::Hamming,
    WindowFunction::BlackmanHarris,
    WindowFunction::FlatTop,
    WindowFunction::Rectangular,
];
pub const FLAT_TOP_COEFFICIENTS: [f32; 5] =
    [0.215_578_95, 0.416_631_58, 0.277_263_16, 0.083_578_944, 0.006_947_368];

pub struct FftKernel {
    fft_size: usize,
    transform: Arc<dyn RealToComplex<f32>>,
    window: Vec<f32>,
    coherent_gain: f32,
    power_normalization: f32,
    input: Vec<f32>,
    output: Vec<realfft::num_complex::Complex32>,
    scratch: Vec<realfft::num_complex::Complex32>,
    amplitudes: Vec<f32>,
    powers: Vec<f32>,
    psd: Vec<f32>,
}

pub struct FftAnalysis<'a> {
    fft_size: usize,
    sample_rate: u32,
    amplitudes: &'a [f32],
    powers: &'a [f32],
    psd: &'a [f32],
}

impl FftAnalysis<'_> {
    pub fn amplitudes(&self) -> &[f32] {
        self.amplitudes
    }

    pub fn powers(&self) -> &[f32] {
        self.powers
    }

    pub fn psd(&self) -> &[f32] {
        self.psd
    }

    pub fn bin_width_hz(&self) -> f32 {
        self.sample_rate as f32 / self.fft_size as f32
    }

    pub fn bin_frequency_hz(&self, index: usize) -> f32 {
        index as f32 * self.bin_width_hz()
    }
}

impl FftKernel {
    pub fn new(fft_size: usize, window_function: WindowFunction) -> Result<Self, FftError> {
        if !SUPPORTED_FFT_SIZES.contains(&fft_size) {
            return Err(FftError::UnsupportedKernelSize { requested: fft_size });
        }
        Ok(Self::plan(fft_size, window_function))
    }

    fn plan(fft_size: usize, window_function: WindowFunction) -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let transform = planner.plan_fft_forward(fft_size);
        let window = make_window(fft_size, window_function);
        let coherent_gain = window.iter().sum::<f32>() / fft_size as f32;
        let power_normalization = window.iter().map(|value| value * value).sum();
        let input = transform.make_input_vec();
        let output = transform.make_output_vec();
        let scratch = transform.make_scratch_vec();
        let bin_count = fft_size / 2 + 1;
        Self {
            fft_size,
            transform,
            window,
            coherent_gain,
            power_normalization,
            input,
            output,
            scratch,
            amplitudes: vec![0.0; bin_count],
            powers: vec![0.0; bin_count],
            psd: vec![0.0; bin_count],
        }
    }

    pub fn coherent_gain(&self) -> f32 {
        self.coherent_gain
    }

    pub fn power_normalization(&self) -> f32 {
        self.power_normalization
    }

    pub fn equivalent_noise_bandwidth_hz(&self, sample_rate: u32) -> Result<f32, FftError> {
        if sample_rate == 0 {
            return Err(FftError::InvalidSampleRate);
        }
        Ok(sample_rate as f32 * self.power_normalization
            / (self.fft_size as f32 * self.coherent_gain).powi(2))
    }

    pub fn analyze(
        &mut self,
        samples: &[f32],
        sample_rate: u32,
    ) -> Result<FftAnalysis<'_>, FftError> {
        if samples.len() != self.fft_size {
            return Err(FftError::FrameSizeMismatch {
                expected: self.fft_size,
                actual: samples.len(),
            });
        }
        if sample_rate == 0 {
            return Err(FftError::InvalidSampleRate);
        }
        if samples.iter().any(|sample| !sample.is_finite()) {
            return Err(FftError::NonFiniteInput);
        }
        for ((input, sample), window) in self.input.iter_mut().zip(samples).zip(self.window.iter())
        {
            *input = sample * window;
        }
        self.transform
            .process_with_scratch(&mut self.input, &mut self.output, &mut self.scratch)
            .map_err(|_| FftError::TransformFailed)?;
        let nyquist = self.fft_size / 2;
        for (index, value) in self.output.iter().enumerate() {
            let one_sided = if index == 0 || index == nyquist { 1.0 } else { 2.0 };
            let norm_squared = value.norm_sqr();
            self.amplitudes[index] =
                value.norm() * one_sided / (self.fft_size as f32 * self.coherent_gain);
            self.powers[index] =
                norm_squared * one_sided / (self.fft_size as f32 * self.power_normalization);
            self.psd[index] =
                norm_squared * one_sided / (sample_rate as f32 * self.power_normalization);
        }
        Ok(FftAnalysis {
            fft_size: self.fft_size,
            sample_rate,
            amplitudes: &self.amplitudes,
            powers: &self.powers,
            psd: &self.psd,
        })
    }
}

fn make_window(fft_size: usize, function: WindowFunction) -> Vec<f32> {
    let denominator = (fft_size - 1) as f32;
    (0..fft_size)
        .map(|index| {
            let phase = std::f32::consts::TAU * index as f32 / denominator;
            match function {
                WindowFunction::Rectangular => 1.0,
                WindowFunction::Hann => 0.5 - 0.5 * phase.cos(),
                WindowFunction::Hamming => 0.54 - 0.46 * phase.cos(),
                WindowFunction::BlackmanHarris => {
                    0.42323 - 0.49755 * phase.cos() + 0.07922 * (2.0 * phase).cos()
                        - 0.00168 * (3.0 * phase).cos()
                },
                WindowFunction::FlatTop => {
                    FLAT_TOP_COEFFICIENTS[0] - FLAT_TOP_COEFFICIENTS[1] * phase.cos()
                        + FLAT_TOP_COEFFICIENTS[2] * (2.0 * phase).cos()
                        - FLAT_TOP_COEFFICIENTS[3] * (3.0 * phase).cos()
                        + FLAT_TOP_COEFFICIENTS[4] * (4.0 * phase).cos()
                },
            }
        })
        .collect()
}

/// Stateful real-to-complex analyzer whose buffers are reused between frames.
pub struct FftAnalyzer {
    kernel: FftKernel,
    mono: Vec<f32>,
}

impl FftAnalyzer {
    pub fn new(fft_size: usize) -> Result<Self, FftError> {
        validate_fft_size(fft_size)?;
        Ok(Self {
            kernel: FftKernel::plan(fft_size, WindowFunction::Hann),
            mono: vec![0.0; fft_size],
        })
    }

    pub fn fft_size(&self) -> usize {
        self.kernel.fft_size
    }

    pub fn analyze(&mut self, frame: &VisualizationFrame) -> Result<SpectrumFrame, FftError> {
        let channels = usize::from(frame.channels());
        let frame_count = frame.samples().len() / channels;
        if frame_count != self.fft_size() {
            return Err(FftError::FrameSizeMismatch {
                expected: self.fft_size(),
                actual: frame_count,
            });
        }

        for (mono, interleaved) in self.mono.iter_mut().zip(frame.samples().chunks_exact(channels))
        {
            *mono = interleaved.iter().copied().sum::<f32>() / channels as f32;
        }
        let fft_size = self.kernel.fft_size;
        let analysis = self.kernel.analyze(&self.mono, frame.sample_rate())?;
        SpectrumFrame::new(
            frame.sequence(),
            frame.position_frames(),
            frame.sample_rate(),
            fft_size,
            analysis.amplitudes().to_vec(),
        )
        .map_err(FftError::InvalidSpectrum)
    }
}

fn validate_fft_size(fft_size: usize) -> Result<(), FftError> {
    if fft_size < 2 || !fft_size.is_power_of_two() || fft_size > MAX_VISUALIZATION_FRAME_SAMPLES {
        return Err(FftError::InvalidFftSize { requested: fft_size });
    }
    Ok(())
}

/// Dedicated visualization-analysis worker. It never runs on the audio callback.
pub struct FftWorker {
    cancel: Arc<AtomicBool>,
    skipped_input: Arc<AtomicU64>,
    dropped_output: Arc<AtomicU64>,
    join: Option<JoinHandle<Result<(), FftError>>>,
    control: Option<VisualizationControl>,
}

impl FftWorker {
    pub fn start(
        subscriber: VisualizationSubscriber,
        fft_size: usize,
        output_capacity: usize,
    ) -> Result<(Self, SpectrumReceiver), FftError> {
        Self::start_inner(subscriber, fft_size, output_capacity, None)
    }

    pub fn start_controlled(
        subscriber: VisualizationSubscriber,
        fft_size: usize,
        output_capacity: usize,
        control: VisualizationControl,
    ) -> Result<(Self, SpectrumReceiver), FftError> {
        Self::start_inner(subscriber, fft_size, output_capacity, Some(control))
    }

    fn start_inner(
        mut subscriber: VisualizationSubscriber,
        fft_size: usize,
        output_capacity: usize,
        control: Option<VisualizationControl>,
    ) -> Result<(Self, SpectrumReceiver), FftError> {
        let mut analyzer = FftAnalyzer::new(fft_size)?;
        if output_capacity == 0 {
            return Err(FftError::InvalidOutputCapacity);
        }
        let (output_tx, output_rx) = mpsc::sync_channel(output_capacity);
        let cancel = Arc::new(AtomicBool::new(false));
        let output_connected = Arc::new(AtomicBool::new(true));
        let skipped_input = Arc::new(AtomicU64::new(0));
        let dropped_output = Arc::new(AtomicU64::new(0));
        let worker_cancel = Arc::clone(&cancel);
        let worker_output_connected = Arc::clone(&output_connected);
        let worker_skipped = Arc::clone(&skipped_input);
        let worker_dropped = Arc::clone(&dropped_output);
        let worker_control = control.clone();
        let join = thread::Builder::new()
            .name("pulseseek-fft".to_string())
            .spawn(move || {
                run_worker(
                    &mut subscriber,
                    &mut analyzer,
                    &output_tx,
                    WorkerSignals {
                        cancel: &worker_cancel,
                        output_connected: &worker_output_connected,
                        skipped_input: &worker_skipped,
                        dropped_output: &worker_dropped,
                        control: worker_control.as_ref(),
                    },
                )
            })
            .map_err(|_| FftError::WorkerStartFailed)?;
        Ok((
            Self {
                cancel,
                skipped_input,
                dropped_output,
                join: Some(join),
                control: control.clone(),
            },
            SpectrumReceiver { receiver: output_rx, connected: output_connected, control },
        ))
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
        if let Some(control) = &self.control {
            control.notify_workers();
        }
    }

    pub fn skipped_input_frames(&self) -> u64 {
        self.skipped_input.load(Ordering::Relaxed)
    }

    pub fn dropped_output_frames(&self) -> u64 {
        self.dropped_output.load(Ordering::Relaxed)
    }

    pub fn wait(mut self) -> Result<(), FftError> {
        self.join
            .take()
            .expect("FFT worker already joined")
            .join()
            .map_err(|_| FftError::WorkerPanicked)?
    }
}

impl Drop for FftWorker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        if let Some(control) = &self.control {
            control.notify_workers();
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

struct WorkerSignals<'a> {
    cancel: &'a AtomicBool,
    output_connected: &'a AtomicBool,
    skipped_input: &'a AtomicU64,
    dropped_output: &'a AtomicU64,
    control: Option<&'a VisualizationControl>,
}

fn run_worker(
    subscriber: &mut VisualizationSubscriber,
    analyzer: &mut FftAnalyzer,
    output: &SyncSender<SpectrumFrame>,
    signals: WorkerSignals<'_>,
) -> Result<(), FftError> {
    loop {
        if signals.cancel.load(Ordering::Acquire) {
            return Err(FftError::Cancelled);
        }
        if !signals.output_connected.load(Ordering::Acquire) {
            return Ok(());
        }
        if let Some(control) = signals.control {
            if !control.is_enabled() {
                while subscriber.try_receive().is_some() {
                    signals.skipped_input.fetch_add(1, Ordering::Relaxed);
                }
                control.wait_while_disabled(signals.cancel, signals.output_connected);
                continue;
            }
        }
        let Some(mut latest) = subscriber.try_receive() else {
            if subscriber.is_closed() {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(1));
            continue;
        };
        while let Some(frame) = subscriber.try_receive() {
            latest = frame;
            signals.skipped_input.fetch_add(1, Ordering::Relaxed);
        }
        if signals.cancel.load(Ordering::Acquire) {
            return Err(FftError::Cancelled);
        }
        let spectrum = analyzer.analyze(&latest)?;
        match output.try_send(spectrum) {
            Ok(()) => {},
            Err(TrySendError::Full(_)) => {
                signals.dropped_output.fetch_add(1, Ordering::Relaxed);
            },
            Err(TrySendError::Disconnected(_)) => return Ok(()),
        }
    }
}

pub struct SpectrumReceiver {
    receiver: Receiver<SpectrumFrame>,
    connected: Arc<AtomicBool>,
    control: Option<VisualizationControl>,
}

pub struct LatestSpectrum {
    pub frame: SpectrumFrame,
    pub discarded: u64,
}

impl SpectrumReceiver {
    pub fn try_receive(&self) -> Option<SpectrumFrame> {
        self.receiver.try_recv().ok()
    }

    /// Returns the newest spectrum currently queued and discards older ones.
    ///
    /// The queue is bounded by the worker output capacity, so this operation
    /// never performs unbounded work. Consumers can use the discard count for
    /// diagnostics without allowing rendering lag to reach playback.
    pub fn try_receive_latest(&self) -> Option<LatestSpectrum> {
        let mut frame = self.try_receive()?;
        let mut discarded = 0_u64;
        while let Some(newer) = self.try_receive() {
            frame = newer;
            discarded = discarded.saturating_add(1);
        }
        Some(LatestSpectrum { frame, discarded })
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Result<SpectrumFrame, RecvTimeoutError> {
        self.receiver.recv_timeout(timeout)
    }
}

impl Drop for SpectrumReceiver {
    fn drop(&mut self) {
        self.connected.store(false, Ordering::Release);
        if let Some(control) = &self.control {
            control.notify_workers();
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FftError {
    InvalidFftSize { requested: usize },
    UnsupportedKernelSize { requested: usize },
    FrameSizeMismatch { expected: usize, actual: usize },
    InvalidSampleRate,
    NonFiniteInput,
    InvalidOutputCapacity,
    TransformFailed,
    InvalidSpectrum(SpectrumFrameError),
    WorkerStartFailed,
    WorkerPanicked,
    Cancelled,
}

impl fmt::Display for FftError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFftSize { requested } => write!(
                formatter,
                "FFT size {requested} must be a power of two between 2 and {MAX_VISUALIZATION_FRAME_SAMPLES}"
            ),
            Self::UnsupportedKernelSize { requested } => {
                write!(formatter, "FFT kernel size {requested} is unsupported")
            },
            Self::FrameSizeMismatch { expected, actual } => {
                write!(formatter, "FFT expected {expected} frames but received {actual}")
            },
            Self::InvalidSampleRate => formatter.write_str("FFT sample rate must be positive"),
            Self::NonFiniteInput => formatter.write_str("FFT input must contain finite samples"),
            Self::InvalidOutputCapacity => formatter.write_str("FFT output capacity must be positive"),
            Self::TransformFailed => formatter.write_str("FFT transform failed"),
            Self::InvalidSpectrum(error) => write!(formatter, "invalid FFT spectrum: {error}"),
            Self::WorkerStartFailed => formatter.write_str("FFT worker could not start"),
            Self::WorkerPanicked => formatter.write_str("FFT worker panicked"),
            Self::Cancelled => formatter.write_str("FFT worker was cancelled"),
        }
    }
}

impl Error for FftError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn spectrum(sequence: u64) -> SpectrumFrame {
        SpectrumFrame::new(sequence, sequence * 256, 48_000, 8, vec![0.0; 5]).unwrap()
    }

    #[test]
    fn kernel_reuses_plan_and_buffers() {
        let size = 2_048;
        let mut kernel = FftKernel::new(size, WindowFunction::Hann).unwrap();
        let plan = Arc::as_ptr(&kernel.transform) as *const () as usize;
        let buffers = [
            kernel.input.as_ptr() as usize,
            kernel.output.as_ptr() as usize,
            kernel.scratch.as_ptr() as usize,
            kernel.amplitudes.as_ptr() as usize,
            kernel.powers.as_ptr() as usize,
            kernel.psd.as_ptr() as usize,
        ];
        let first: Vec<f32> = (0..size)
            .map(|index| (std::f32::consts::TAU * 20.0 * index as f32 / size as f32).sin())
            .collect();
        let second: Vec<f32> = (0..size)
            .map(|index| (std::f32::consts::TAU * 30.0 * index as f32 / size as f32).sin())
            .collect();

        kernel.analyze(&first, 48_000).unwrap();
        kernel.analyze(&second, 48_000).unwrap();

        assert_eq!(Arc::as_ptr(&kernel.transform) as *const () as usize, plan);
        assert_eq!(
            [
                kernel.input.as_ptr() as usize,
                kernel.output.as_ptr() as usize,
                kernel.scratch.as_ptr() as usize,
                kernel.amplitudes.as_ptr() as usize,
                kernel.powers.as_ptr() as usize,
                kernel.psd.as_ptr() as usize,
            ],
            buffers
        );
    }

    #[test]
    fn latest_receive_discards_queued_spectra() {
        let (sender, receiver) = mpsc::sync_channel(3);
        sender.try_send(spectrum(1)).unwrap();
        sender.try_send(spectrum(2)).unwrap();
        sender.try_send(spectrum(3)).unwrap();
        let connected = Arc::new(AtomicBool::new(true));
        let spectra = SpectrumReceiver { receiver, connected, control: None };

        let latest = spectra.try_receive_latest().expect("latest spectrum");

        assert_eq!(latest.frame.sequence(), 3);
        assert_eq!(latest.discarded, 2);
        assert!(spectra.try_receive().is_none());
    }
}
