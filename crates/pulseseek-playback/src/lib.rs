mod resampling;

mod analysis_capture;
mod analysis_event_runtime;
mod analysis_lanes;
mod control;
mod engine;
mod error;
mod event;
mod fft;
mod fft_bank;
mod musical_spectrum;
mod visualization;
mod worker;

pub use analysis_capture::*;
pub use analysis_event_runtime::{
    AnalysisEventRuntime, EventEnvelope, EventReceiver, MeteringPublishResult,
};
pub use analysis_lanes::*;
pub use control::*;
pub use engine::*;
pub use error::*;
pub use event::*;
pub use fft::*;
pub use fft_bank::*;
pub use musical_spectrum::*;
pub use visualization::*;
pub use worker::*;

/// Applies linear gain to one sample and hard-clips the result to audio range.
///
/// Intended for the real-time output callback: constant-time arithmetic only.
pub fn apply_volume(sample: f32, gain: f32) -> f32 {
    (sample * gain).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests;
