use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::{FormatOptions, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

use pulseseek_domain::decoder::{DecodeError, Decoder, ProbeResult, StreamMetadata};
use pulseseek_domain::error::{DiagnosticCode, DiagnosticContext};
use pulseseek_domain::playback::position::{Duration, Position, SeekTarget};

/// Shared internal state for Symphonia-based decoders.
struct SymphoniaDecoder {
    format: Box<dyn symphonia::core::formats::FormatReader>,
    decoder: Box<dyn symphonia::core::codecs::Decoder>,
    track_id: u32,
    sample_rate: u32,
    channels: u16,
    duration_ms: u64,
    sample_buf: SampleBuffer<f32>,
}

impl SymphoniaDecoder {
    fn open(path: impl AsRef<Path>, extension: &str) -> Result<Self, DecodeError> {
        let file = std::fs::File::open(path.as_ref()).map_err(|e| {
            DecodeError::new(DiagnosticContext::new(DiagnosticCode::BrowserRead), e)
        })?;

        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        hint.with_extension(extension);

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
            .map_err(|e| {
                DecodeError::new(DiagnosticContext::new(DiagnosticCode::BrowserRead), e)
            })?;

        let format = probed.format;

        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
            .ok_or_else(|| {
                DecodeError::new(
                    DiagnosticContext::new(DiagnosticCode::BrowserRead),
                    std::io::Error::other("no audio track found"),
                )
            })?;

        let track_id = track.id;
        let codec_params = &track.codec_params;
        let sample_rate = codec_params.sample_rate.unwrap_or(0);
        let channels = codec_params.channels.map(|c| c.count() as u16).unwrap_or(0);

        let duration_ms = codec_params
            .n_frames
            .zip(codec_params.sample_rate)
            .map(|(frames, rate)| (frames * 1000) / rate as u64)
            .unwrap_or(0);

        let dec_opts = DecoderOptions { verify: true };
        let decoder =
            symphonia::default::get_codecs().make(&track.codec_params, &dec_opts).map_err(|e| {
                DecodeError::new(DiagnosticContext::new(DiagnosticCode::BrowserRead), e)
            })?;

        let spec = symphonia::core::audio::SignalSpec::new(
            sample_rate,
            codec_params.channels.unwrap_or(
                symphonia::core::audio::Channels::FRONT_LEFT
                    | symphonia::core::audio::Channels::FRONT_RIGHT,
            ),
        );
        let cap_frames: u64 = 65536;
        let sample_buf = SampleBuffer::<f32>::new(cap_frames, spec);

        Ok(Self { format, decoder, track_id, sample_rate, channels, duration_ms, sample_buf })
    }
}

impl Decoder for SymphoniaDecoder {
    fn probe(&self) -> ProbeResult {
        ProbeResult::Supported
    }

    fn metadata(&mut self) -> Result<StreamMetadata, DecodeError> {
        Ok(StreamMetadata {
            sample_rate: self.sample_rate,
            channels: self.channels,
            duration: Duration::from_millis(self.duration_ms),
        })
    }

    fn read(&mut self, buf: &mut [f32]) -> Result<usize, DecodeError> {
        let mut total_written = 0usize;

        while total_written < buf.len() {
            let packet = match self.format.next_packet() {
                Ok(pkt) => pkt,
                Err(symphonia::core::errors::Error::IoError(ref e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                },
                Err(e) => {
                    return Err(DecodeError::new(
                        DiagnosticContext::new(DiagnosticCode::BrowserRead),
                        e,
                    ));
                },
            };

            let decoded = self.decoder.decode(&packet).map_err(|e| {
                DecodeError::new(DiagnosticContext::new(DiagnosticCode::BrowserRead), e)
            })?;

            self.sample_buf.clear();
            self.sample_buf.copy_interleaved_ref(decoded);

            let samples = self.sample_buf.samples();
            let to_copy = samples.len().min(buf.len() - total_written);
            buf[total_written..total_written + to_copy].copy_from_slice(&samples[..to_copy]);
            total_written += to_copy;

            if to_copy < samples.len() {
                break;
            }
        }

        Ok(total_written)
    }

    fn seek(&mut self, target: SeekTarget) -> Result<Position, DecodeError> {
        let total_ms = target.position().as_millis();
        let seconds = total_ms / 1000;
        let frac = (total_ms % 1000) as f64 / 1000.0;

        self.format
            .seek(
                SeekMode::Accurate,
                SeekTo::Time { time: Time { seconds, frac }, track_id: Some(self.track_id) },
            )
            .map_err(|e| {
                DecodeError::new(DiagnosticContext::new(DiagnosticCode::BrowserRead), e)
            })?;

        Ok(target.position())
    }
}

/// A Symphonia-based decoder for WAV audio files.
pub struct WavDecoder(SymphoniaDecoder);

impl WavDecoder {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DecodeError> {
        SymphoniaDecoder::open(path, "wav").map(WavDecoder)
    }
}

impl Decoder for WavDecoder {
    fn probe(&self) -> ProbeResult {
        self.0.probe()
    }
    fn metadata(&mut self) -> Result<StreamMetadata, DecodeError> {
        self.0.metadata()
    }
    fn read(&mut self, buf: &mut [f32]) -> Result<usize, DecodeError> {
        self.0.read(buf)
    }
    fn seek(&mut self, target: SeekTarget) -> Result<Position, DecodeError> {
        self.0.seek(target)
    }
}

/// A Symphonia-based decoder for FLAC audio files.
pub struct FlacDecoder(SymphoniaDecoder);

impl FlacDecoder {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DecodeError> {
        SymphoniaDecoder::open(path, "flac").map(FlacDecoder)
    }
}

impl Decoder for FlacDecoder {
    fn probe(&self) -> ProbeResult {
        self.0.probe()
    }
    fn metadata(&mut self) -> Result<StreamMetadata, DecodeError> {
        self.0.metadata()
    }
    fn read(&mut self, buf: &mut [f32]) -> Result<usize, DecodeError> {
        self.0.read(buf)
    }
    fn seek(&mut self, target: SeekTarget) -> Result<Position, DecodeError> {
        self.0.seek(target)
    }
}

/// A Symphonia-based decoder for MP3 audio files.
pub struct Mp3Decoder(SymphoniaDecoder);

impl Mp3Decoder {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DecodeError> {
        SymphoniaDecoder::open(path, "mp3").map(Mp3Decoder)
    }
}

impl Decoder for Mp3Decoder {
    fn probe(&self) -> ProbeResult {
        self.0.probe()
    }
    fn metadata(&mut self) -> Result<StreamMetadata, DecodeError> {
        self.0.metadata()
    }
    fn read(&mut self, buf: &mut [f32]) -> Result<usize, DecodeError> {
        self.0.read(buf)
    }
    fn seek(&mut self, target: SeekTarget) -> Result<Position, DecodeError> {
        self.0.seek(target)
    }
}

pub mod registry;
