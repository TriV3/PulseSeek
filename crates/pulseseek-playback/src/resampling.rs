use std::collections::VecDeque;

use pulseseek_domain::decoder::{DecodeError, Decoder};
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Async, FixedAsync, Indexing, PolynomialDegree, Resampler};

/// Worker-side sample-rate conversion state.
pub(crate) struct SampleRateConverter {
    channels: usize,
    source_rate: u32,
    target_rate: u32,
    resampler: Async<f32>,
    input: Vec<f32>,
    output: Vec<f32>,
    pending: VecDeque<f32>,
    source_frames: u64,
    output_samples: u64,
}

impl SampleRateConverter {
    pub(crate) fn validate(
        channels: usize,
        source_rate: u32,
        target_rate: u32,
    ) -> Result<(), DecodeError> {
        if channels == 0 || source_rate == 0 || target_rate == 0 {
            return Err(converter_error("resampler configuration must be positive"));
        }
        Ok(())
    }

    pub(crate) fn new(
        channels: usize,
        source_rate: u32,
        target_rate: u32,
    ) -> Result<Self, DecodeError> {
        Self::validate(channels, source_rate, target_rate)?;

        let input_frames = 256;
        let resampler = Async::new_poly(
            target_rate as f64 / source_rate as f64,
            1.0,
            PolynomialDegree::Cubic,
            input_frames,
            channels,
            FixedAsync::Input,
        )
        .map_err(|error| converter_error(&error.to_string()))?;
        let input = vec![0.0; channels * resampler.input_frames_next()];
        let output = vec![0.0; channels * resampler.output_frames_max()];

        Ok(Self {
            channels,
            source_rate,
            target_rate,
            resampler,
            input,
            output,
            pending: VecDeque::new(),
            source_frames: 0,
            output_samples: 0,
        })
    }

    pub(crate) fn next_chunk(
        &mut self,
        decoder: &mut dyn Decoder,
    ) -> Result<Option<Vec<f32>>, DecodeError> {
        if !self.pending.is_empty() {
            return Ok(Some(self.drain_pending()));
        }

        let mut filled = 0;
        while filled < self.input.len() {
            let read = decoder.read(&mut self.input[filled..])?;
            if read == 0 {
                break;
            }
            filled += read;
        }

        if filled == 0 {
            return Ok(None);
        }
        if filled % self.channels != 0 {
            return Err(converter_error("decoder returned partial interleaved frame"));
        }

        let input_frames = filled / self.channels;
        self.source_frames += input_frames as u64;
        let input =
            InterleavedSlice::new(&self.input, self.channels, self.input.len() / self.channels)
                .map_err(|error| converter_error(&error.to_string()))?;
        let output_frames = self.output.len() / self.channels;
        let mut output = InterleavedSlice::new_mut(&mut self.output, self.channels, output_frames)
            .map_err(|error| converter_error(&error.to_string()))?;
        let indexing = Indexing::new().partial_len(input_frames);
        let (_, frames_written) = self
            .resampler
            .process_into_buffer(&input, &mut output, Some(&indexing))
            .map_err(|error| converter_error(&error.to_string()))?;

        let expected_total = ((self.source_frames as u128 * self.target_rate as u128)
            / self.source_rate as u128) as u64
            * self.channels as u64;
        let produced = (frames_written * self.channels) as u64;
        let remaining = expected_total.saturating_sub(self.output_samples);
        let keep = produced.min(remaining) as usize;
        self.pending.extend(self.output[..keep].iter().copied());
        self.output_samples += keep as u64;

        Ok(Some(self.drain_pending()))
    }

    fn drain_pending(&mut self) -> Vec<f32> {
        self.pending.drain(..).collect()
    }

    /// Decoder (source) position in milliseconds.
    pub(crate) fn source_position_ms(&self) -> u64 {
        if self.source_rate == 0 || self.channels == 0 {
            return 0;
        }
        self.source_frames.saturating_mul(1_000) / u64::from(self.source_rate)
    }

    pub(crate) fn reset(&mut self) {
        self.resampler.reset();
        self.input.fill(0.0);
        self.output.fill(0.0);
        self.pending.clear();
        self.source_frames = 0;
        self.output_samples = 0;
    }
}

fn converter_error(message: &str) -> DecodeError {
    DecodeError::new(
        pulseseek_domain::error::DiagnosticContext::new(
            pulseseek_domain::error::DiagnosticCode::AudioOutput,
        ),
        std::io::Error::other(message.to_owned()),
    )
}

#[cfg(test)]
mod tests {
    use super::SampleRateConverter;

    #[test]
    fn stereo_source_position_counts_frames_once() {
        let mut converter = SampleRateConverter::new(2, 1_000, 2_000).unwrap();
        converter.source_frames = 500;

        assert_eq!(converter.source_position_ms(), 500);
    }
}
