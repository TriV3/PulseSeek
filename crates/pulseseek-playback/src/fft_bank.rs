use std::collections::HashMap;

use pulseseek_domain::analysis_subscriptions::{ChannelMode, WindowFunction};
use realfft::num_complex::Complex32;

use crate::{FftError, FftKernel};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FftBranchKey {
    fft_size: usize,
    window: WindowFunction,
}

impl FftBranchKey {
    pub const fn new(fft_size: usize, window: WindowFunction) -> Self {
        Self { fft_size, window }
    }

    pub const fn fft_size(self) -> usize {
        self.fft_size
    }

    pub const fn window(self) -> WindowFunction {
        self.window
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FftStreamId(u64);

impl FftStreamId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FftSubscriptionId(u64);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FftBranchId(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FftBankSubscription {
    id: FftSubscriptionId,
    branch_id: FftBranchId,
}

impl FftBankSubscription {
    pub const fn id(self) -> FftSubscriptionId {
        self.id
    }

    pub const fn branch_id(self) -> FftBranchId {
        self.branch_id
    }
}

struct FftBranch {
    id: FftBranchId,
    consumers: usize,
    left_kernel: FftKernel,
    right_kernel: FftKernel,
    left_samples: Vec<f32>,
    right_samples: Vec<f32>,
    left_bins: Vec<Complex32>,
    right_bins: Vec<Complex32>,
    left: Vec<f32>,
    right: Vec<f32>,
    energy_sum: Vec<f32>,
    mono: Vec<f32>,
    mid: Vec<f32>,
    side: Vec<f32>,
    difference: Vec<f32>,
    balance: Vec<Option<f32>>,
    pending_left: Vec<f32>,
    pending_left_bins: Vec<Complex32>,
    last_frame_id: Option<u64>,
    last_sample_rate: Option<u32>,
    last_frame_fingerprint: Option<u64>,
    transform_count: u64,
}

impl FftBranch {
    fn new(id: FftBranchId, key: FftBranchKey) -> Result<Self, FftError> {
        let bin_count = key.fft_size / 2 + 1;
        Ok(Self {
            id,
            consumers: 0,
            left_kernel: FftKernel::new(key.fft_size, key.window)?,
            right_kernel: FftKernel::new(key.fft_size, key.window)?,
            left_samples: vec![0.0; key.fft_size],
            right_samples: vec![0.0; key.fft_size],
            left_bins: vec![Complex32::default(); bin_count],
            right_bins: vec![Complex32::default(); bin_count],
            left: vec![0.0; bin_count],
            right: vec![0.0; bin_count],
            energy_sum: vec![0.0; bin_count],
            mono: vec![0.0; bin_count],
            mid: vec![0.0; bin_count],
            side: vec![0.0; bin_count],
            difference: vec![0.0; bin_count],
            balance: vec![None; bin_count],
            pending_left: vec![0.0; bin_count],
            pending_left_bins: vec![Complex32::default(); bin_count],
            last_frame_id: None,
            last_sample_rate: None,
            last_frame_fingerprint: None,
            transform_count: 0,
        })
    }

    fn process(
        &mut self,
        frame_id: u64,
        samples: &[f32],
        sample_rate: u32,
    ) -> Result<(), FftError> {
        let expected = self.left_samples.len() * 2;
        if samples.len() != expected {
            return Err(FftError::InterleavedFrameSizeMismatch { expected, actual: samples.len() });
        }
        if sample_rate == 0 {
            return Err(FftError::InvalidSampleRate);
        }
        if samples.iter().any(|sample| !sample.is_finite()) {
            return Err(FftError::NonFiniteInput);
        }
        let fingerprint = frame_fingerprint(samples, sample_rate);
        if let Some(latest) = self.last_frame_id {
            if frame_id < latest {
                return Err(FftError::StaleFrame { latest, received: frame_id });
            }
            if frame_id == latest {
                return if self.last_frame_fingerprint == Some(fingerprint) {
                    Ok(())
                } else {
                    Err(FftError::ConflictingFrameIdentity { frame_id })
                };
            }
        }
        for ((left, right), frame) in
            self.left_samples.iter_mut().zip(&mut self.right_samples).zip(samples.chunks_exact(2))
        {
            *left = frame[0];
            *right = frame[1];
        }

        let left_analysis = self.left_kernel.analyze(&self.left_samples, sample_rate)?;
        self.pending_left.copy_from_slice(left_analysis.amplitudes());
        self.pending_left_bins.copy_from_slice(left_analysis.complex_bins());
        let right_analysis = self.right_kernel.analyze(&self.right_samples, sample_rate)?;
        self.right.copy_from_slice(right_analysis.amplitudes());
        self.right_bins.copy_from_slice(right_analysis.complex_bins());
        self.left.copy_from_slice(&self.pending_left);
        self.left_bins.copy_from_slice(&self.pending_left_bins);

        let fft_size = self.left_samples.len();
        let coherent_gain = self.left_kernel.coherent_gain();
        let nyquist = fft_size / 2;
        for index in 0..self.left.len() {
            let one_sided = if index == 0 || index == nyquist { 1.0 } else { 2.0 };
            let scale = one_sided / (fft_size as f32 * coherent_gain);
            let left = self.left_bins[index];
            let right = self.right_bins[index];
            let left_amplitude = self.left[index];
            let right_amplitude = self.right[index];
            self.energy_sum[index] =
                ((left_amplitude.powi(2) + right_amplitude.powi(2)) / 2.0).sqrt();
            self.mono[index] = ((left + right) * 0.5).norm() * scale;
            self.mid[index] = ((left + right) / std::f32::consts::SQRT_2).norm() * scale;
            self.side[index] = ((left - right) / std::f32::consts::SQRT_2).norm() * scale;
            self.difference[index] = left_amplitude - right_amplitude;
            let denominator = left_amplitude + right_amplitude;
            self.balance[index] = (denominator > f32::EPSILON)
                .then_some((left_amplitude - right_amplitude) / denominator);
        }
        self.last_frame_id = Some(frame_id);
        self.last_sample_rate = Some(sample_rate);
        self.last_frame_fingerprint = Some(fingerprint);
        self.transform_count = self.transform_count.saturating_add(1);
        Ok(())
    }
}

fn frame_fingerprint(samples: &[f32], sample_rate: u32) -> u64 {
    samples.iter().fold(u64::from(sample_rate), |hash, sample| {
        hash.wrapping_mul(1_099_511_628_211).wrapping_add(u64::from(sample.to_bits()))
    })
}

pub struct FftBank {
    stream_id: FftStreamId,
    next_subscription_id: u64,
    next_branch_id: u64,
    branches: HashMap<FftBranchKey, FftBranch>,
    subscriptions: HashMap<FftSubscriptionId, FftBranchKey>,
}

impl FftBank {
    pub fn new(stream_id: FftStreamId) -> Self {
        Self {
            stream_id,
            next_subscription_id: 0,
            next_branch_id: 0,
            branches: HashMap::new(),
            subscriptions: HashMap::new(),
        }
    }

    pub fn subscribe(&mut self, key: FftBranchKey) -> Result<FftBankSubscription, FftError> {
        if !self.branches.contains_key(&key) {
            self.next_branch_id =
                self.next_branch_id.checked_add(1).ok_or(FftError::IdExhausted)?;
            let id = FftBranchId(self.next_branch_id);
            self.branches.insert(key, FftBranch::new(id, key)?);
        }
        self.next_subscription_id =
            self.next_subscription_id.checked_add(1).ok_or(FftError::IdExhausted)?;
        let id = FftSubscriptionId(self.next_subscription_id);
        let branch = self.branches.get_mut(&key).expect("FFT branch registered");
        branch.consumers += 1;
        let branch_id = branch.id;
        self.subscriptions.insert(id, key);
        Ok(FftBankSubscription { id, branch_id })
    }

    pub fn unsubscribe(&mut self, id: FftSubscriptionId) -> bool {
        let Some(key) = self.subscriptions.remove(&id) else { return false };
        let branch = self.branches.get_mut(&key).expect("subscribed FFT branch exists");
        branch.consumers -= 1;
        if branch.consumers == 0 {
            self.branches.remove(&key);
        }
        true
    }

    pub fn process(
        &mut self,
        id: FftSubscriptionId,
        frame_id: u64,
        stereo_samples: &[f32],
        sample_rate: u32,
    ) -> Result<(), FftError> {
        let key = *self.subscriptions.get(&id).ok_or(FftError::UnknownSubscription)?;
        self.branches.get_mut(&key).expect("subscribed FFT branch exists").process(
            frame_id,
            stereo_samples,
            sample_rate,
        )
    }

    pub fn analysis(&self, id: FftSubscriptionId) -> Result<FftBankAnalysis<'_>, FftError> {
        let key = *self.subscriptions.get(&id).ok_or(FftError::UnknownSubscription)?;
        let branch = self.branches.get(&key).expect("subscribed FFT branch exists");
        if branch.last_frame_id.is_none() {
            return Err(FftError::AnalysisUnavailable);
        }
        Ok(FftBankAnalysis { stream_id: self.stream_id, key, branch })
    }

    pub fn active_branch_count(&self) -> usize {
        self.branches.len()
    }

    pub fn active_plan_count(&self) -> usize {
        self.branches.len() * 2
    }

    pub fn consumer_count(&self, key: FftBranchKey) -> usize {
        self.branches.get(&key).map_or(0, |branch| branch.consumers)
    }

    pub fn branch_transform_count(&self, key: FftBranchKey) -> u64 {
        self.branches.get(&key).map_or(0, |branch| branch.transform_count)
    }
}

pub struct FftBankAnalysis<'a> {
    stream_id: FftStreamId,
    key: FftBranchKey,
    branch: &'a FftBranch,
}

impl FftBankAnalysis<'_> {
    pub const fn stream_id(&self) -> FftStreamId {
        self.stream_id
    }

    pub fn frame_id(&self) -> u64 {
        self.branch.last_frame_id.expect("available FFT analysis has a frame identity")
    }

    pub fn sample_rate(&self) -> u32 {
        self.branch.last_sample_rate.expect("available FFT analysis has a sample rate")
    }

    pub fn bin_frequency_hz(&self, index: usize) -> f32 {
        index as f32 * self.sample_rate() as f32 / self.fft_size() as f32
    }

    pub const fn fft_size(&self) -> usize {
        self.key.fft_size
    }

    pub const fn window(&self) -> WindowFunction {
        self.key.window
    }

    pub fn left_right_overlay(&self) -> (&[f32], &[f32]) {
        (&self.branch.left, &self.branch.right)
    }

    pub fn left_right_difference(&self) -> &[f32] {
        &self.branch.difference
    }

    pub fn left_right_balance(&self) -> &[Option<f32>] {
        &self.branch.balance
    }

    pub fn amplitudes(&self, mode: ChannelMode) -> Option<&[f32]> {
        match mode {
            ChannelMode::Left => Some(&self.branch.left),
            ChannelMode::Right => Some(&self.branch.right),
            ChannelMode::EnergySum => Some(&self.branch.energy_sum),
            ChannelMode::Mono => Some(&self.branch.mono),
            ChannelMode::Mid => Some(&self.branch.mid),
            ChannelMode::Side => Some(&self.branch.side),
            ChannelMode::Stereo | ChannelMode::LeftRightOverlay | ChannelMode::LeftRightBalance => {
                None
            },
        }
    }
}
