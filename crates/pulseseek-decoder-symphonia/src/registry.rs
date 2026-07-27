use std::path::Path;

use pulseseek_domain::decoder::{DecodeError, Decoder};

use crate::{FlacDecoder, Mp3Decoder, WavDecoder};

/// Registry that selects the correct decoder based on content probing.
///
/// Tries each registered decoder in order (MP3 → FLAC → WAV) and returns
/// the first that successfully probes and opens the file. Selection is
/// based on content headers, not file extension.
pub struct DecoderRegistry;

impl DecoderRegistry {
    /// Opens a file by probing with each registered decoder.
    ///
    /// Returns `Err` if no decoder supports the content.
    pub fn open(path: impl AsRef<Path>) -> Result<Box<dyn Decoder>, DecodeError> {
        let path = path.as_ref();

        // Try each decoder; MP3 first (most distinctive header).
        if let Ok(d) = Mp3Decoder::open(path) {
            return Ok(Box::new(d));
        }
        if let Ok(d) = FlacDecoder::open(path) {
            return Ok(Box::new(d));
        }
        if let Ok(d) = WavDecoder::open(path) {
            return Ok(Box::new(d));
        }

        Err(DecodeError::new(
            pulseseek_domain::error::DiagnosticContext::new(
                pulseseek_domain::error::DiagnosticCode::BrowserRead,
            ),
            std::io::Error::other("no decoder supports this file"),
        ))
    }
}
