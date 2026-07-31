mod resampling;

mod control;
mod engine;
mod error;
mod event;
mod worker;

pub use control::*;
pub use engine::*;
pub use error::*;
pub use event::*;
pub use worker::*;

/// Applies linear gain to one sample and hard-clips the result to audio range.
///
/// Intended for the real-time output callback: constant-time arithmetic only.
pub fn apply_volume(sample: f32, gain: f32) -> f32 {
    (sample * gain).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests;
