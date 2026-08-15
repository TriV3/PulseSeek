use std::path::Path;

use pulseseek_decoder_symphonia::probe_stream_metadata;
use pulseseek_domain::browser::probe::{ProbeError, ProbeFile, ProbeResult};

/// Native filesystem adapter for [`ProbeFile`].
///
/// Classifies a dropped path by reading metadata and, for audio-looking files,
/// probing the stream header. `NotFound` maps to [`ProbeResult::Missing`]
/// (a normal drop outcome), while other metadata failures surface as a
/// [`ProbeError`] so the UI can report the inspection problem.
pub struct NativeProbe;

impl ProbeFile for NativeProbe {
    fn probe(&self, path: &Path) -> Result<ProbeResult, ProbeError> {
        let metadata = match std::fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ProbeResult::Missing);
            },
            Err(error) => return Err(ProbeError::from_io_error(error, path)),
        };

        if metadata.is_dir() {
            return Ok(ProbeResult::Directory);
        }

        // Skip decoding work for targets that can never be audio. The
        // allow-list stays identical to the browser entry classification.
        if !crate::likely_supported_audio(path) {
            return Ok(ProbeResult::Unsupported);
        }

        match probe_stream_metadata(path) {
            Ok(_) => Ok(ProbeResult::Playable),
            Err(_) => Ok(ProbeResult::Unsupported),
        }
    }
}

#[cfg(test)]
mod tests;
