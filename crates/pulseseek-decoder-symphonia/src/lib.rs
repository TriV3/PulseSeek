use std::collections::VecDeque;
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
    bit_depth: Option<u32>,
    codec_name: &'static str,
    sample_buf: SampleBuffer<f32>,
    pending_samples: VecDeque<f32>,
    discard_until_ts: Option<u64>,
}

impl SymphoniaDecoder {
    fn open(path: impl AsRef<Path>, _extension: &str) -> Result<Self, DecodeError> {
        let file = std::fs::File::open(path.as_ref()).map_err(|e| {
            DecodeError::new(DiagnosticContext::new(DiagnosticCode::BrowserRead), e)
        })?;

        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        // Do not force a format from the filename extension. The browser
        // relies on this probe to distinguish actual audio from installers or
        // disk images that happen to use a misleading name.
        let hint = Hint::new();

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

        let codec = track.codec_params.codec;
        let bit_depth = track.codec_params.bits_per_sample;
        let codec_name = codec_name(codec);

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

        Ok(Self {
            format,
            decoder,
            track_id,
            sample_rate,
            channels,
            duration_ms,
            bit_depth,
            codec_name,
            sample_buf,
            pending_samples: VecDeque::new(),
            discard_until_ts: None,
        })
    }

    fn seek_with_mode(
        &mut self,
        target: SeekTarget,
        mode: SeekMode,
        discard_until_required_ts: bool,
    ) -> Result<Position, DecodeError> {
        let total_ms = target.position().as_millis();
        let seconds = total_ms / 1000;
        let frac = (total_ms % 1000) as f64 / 1000.0;

        let seeked_to = self
            .format
            .seek(
                mode,
                SeekTo::Time { time: Time { seconds, frac }, track_id: Some(self.track_id) },
            )
            .map_err(|e| {
                DecodeError::new(DiagnosticContext::new(DiagnosticCode::BrowserRead), e)
            })?;

        self.pending_samples.clear();
        self.decoder.reset();
        self.discard_until_ts = discard_until_required_ts.then_some(seeked_to.required_ts);

        Ok(target.position())
    }
}

/// Returns true for known PCM codec types (range check).
fn is_pcm_codec(codec: symphonia::core::codecs::CodecType) -> bool {
    codec == symphonia::core::codecs::CODEC_TYPE_PCM_S16LE
        || codec == symphonia::core::codecs::CODEC_TYPE_PCM_S16BE
        || codec == symphonia::core::codecs::CODEC_TYPE_PCM_S24LE
        || codec == symphonia::core::codecs::CODEC_TYPE_PCM_S24BE
        || codec == symphonia::core::codecs::CODEC_TYPE_PCM_S32LE
        || codec == symphonia::core::codecs::CODEC_TYPE_PCM_S32BE
        || codec == symphonia::core::codecs::CODEC_TYPE_PCM_F32LE
        || codec == symphonia::core::codecs::CODEC_TYPE_PCM_F32BE
        || codec == symphonia::core::codecs::CODEC_TYPE_PCM_S16LE_PLANAR
        || codec == symphonia::core::codecs::CODEC_TYPE_PCM_S24LE_PLANAR
        || codec == symphonia::core::codecs::CODEC_TYPE_PCM_S32LE_PLANAR
        || codec == symphonia::core::codecs::CODEC_TYPE_PCM_F32LE_PLANAR
        || codec == symphonia::core::codecs::CODEC_TYPE_PCM_U8
        || codec == symphonia::core::codecs::CODEC_TYPE_PCM_S8
}

fn codec_name(codec: symphonia::core::codecs::CodecType) -> &'static str {
    if is_pcm_codec(codec) {
        "PCM"
    } else if codec == symphonia::core::codecs::CODEC_TYPE_FLAC {
        "FLAC"
    } else if codec == symphonia::core::codecs::CODEC_TYPE_MP3 {
        "MP3"
    } else {
        "Unknown"
    }
}

/// Reads stream metadata through Symphonia's format probe without constructing
/// an audio decoder or allocating decode buffers. The filename extension is a
/// probe hint only; unsupported content is still rejected by its headers.
pub fn probe_stream_metadata(path: impl AsRef<Path>) -> Result<StreamMetadata, DecodeError> {
    let path = path.as_ref();
    let file = std::fs::File::open(path).map_err(|error| {
        DecodeError::new(DiagnosticContext::new(DiagnosticCode::BrowserRead), error)
    })?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
        hint.with_extension(extension);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|error| {
            DecodeError::new(DiagnosticContext::new(DiagnosticCode::BrowserRead), error)
        })?;
    let track = probed
        .format
        .tracks()
        .iter()
        .find(|track| track.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .ok_or_else(|| {
            DecodeError::new(
                DiagnosticContext::new(DiagnosticCode::BrowserRead),
                std::io::Error::other("no audio track found"),
            )
        })?;
    let params = &track.codec_params;
    let sample_rate = params.sample_rate.unwrap_or(0);
    let duration_ms = params
        .n_frames
        .zip(params.sample_rate)
        .map(|(frames, rate)| (frames * 1000) / u64::from(rate))
        .unwrap_or(0);

    Ok(StreamMetadata {
        sample_rate,
        channels: params.channels.map(|channels| channels.count() as u16).unwrap_or(0),
        duration: Duration::from_millis(duration_ms),
        bit_depth: params.bits_per_sample,
        codec: codec_name(params.codec),
    })
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
            bit_depth: self.bit_depth,
            codec: self.codec_name,
        })
    }

    fn read(&mut self, buf: &mut [f32]) -> Result<usize, DecodeError> {
        let mut total_written = 0usize;

        while total_written < buf.len() {
            let Some(sample) = self.pending_samples.pop_front() else {
                break;
            };
            buf[total_written] = sample;
            total_written += 1;
        }

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
            let sample_offset = self
                .discard_until_ts
                .map(|required_ts| required_ts.saturating_sub(packet.ts()) as usize)
                .unwrap_or(0)
                .saturating_mul(self.channels as usize)
                .min(samples.len());
            if sample_offset < samples.len() {
                self.discard_until_ts = None;
            }
            let audible_samples = &samples[sample_offset..];
            let to_copy = audible_samples.len().min(buf.len() - total_written);
            buf[total_written..total_written + to_copy]
                .copy_from_slice(&audible_samples[..to_copy]);
            total_written += to_copy;

            if to_copy < audible_samples.len() {
                self.pending_samples.extend(audible_samples[to_copy..].iter().copied());
                break;
            }
        }

        Ok(total_written)
    }

    fn seek(&mut self, target: SeekTarget) -> Result<Position, DecodeError> {
        self.seek_with_mode(target, SeekMode::Accurate, true)
    }

    fn seek_coarse(&mut self, target: SeekTarget) -> Result<Position, DecodeError> {
        self.seek_with_mode(target, SeekMode::Coarse, false)
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
    fn seek_coarse(&mut self, target: SeekTarget) -> Result<Position, DecodeError> {
        self.0.seek_coarse(target)
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
    fn seek_coarse(&mut self, target: SeekTarget) -> Result<Position, DecodeError> {
        self.0.seek_coarse(target)
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
    fn seek_coarse(&mut self, target: SeekTarget) -> Result<Position, DecodeError> {
        self.0.seek_coarse(target)
    }
}

pub mod registry;
pub mod waveform;
