pub mod analysis;
pub mod audio_output;
pub mod browser;
pub mod decoder;
pub mod error;
pub mod playback;
pub mod shortcuts;
pub mod visualization;
pub mod waveform;

pub fn placeholder() -> &'static str {
    "PulseSeek"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_returns_app_name() {
        assert_eq!(placeholder(), "PulseSeek");
    }
}
