use std::error::Error;
use std::fmt;

use pulseseek_domain::visualization::{
    MusicalBand, MusicalSpectrumFrame, MusicalSpectrumFrameError, SpectrumFrame,
};

pub const DEFAULT_TUNING_REFERENCE_HZ: f32 = 440.0;
const FIRST_NOTE_NUMBER: i16 = 12;
const A4_NOTE_NUMBER: f32 = 69.0;
const NOTES_PER_OCTAVE: f32 = 12.0;

/// Converts FFT-bin energy into contiguous twelve-tone equal-tempered bands.
#[derive(Clone, Copy, Debug)]
pub struct MusicalSpectrumAnalyzer {
    tuning_reference_hz: f32,
}

impl MusicalSpectrumAnalyzer {
    pub fn new(tuning_reference_hz: f32) -> Result<Self, MusicalSpectrumError> {
        if !tuning_reference_hz.is_finite() || tuning_reference_hz <= 0.0 {
            return Err(MusicalSpectrumError::InvalidTuningReference);
        }
        Ok(Self { tuning_reference_hz })
    }

    pub fn analyze(
        &self,
        spectrum: &SpectrumFrame,
    ) -> Result<MusicalSpectrumFrame, MusicalSpectrumError> {
        let nyquist_hz = spectrum.sample_rate() as f32 / 2.0;
        let definitions = self.band_definitions(nyquist_hz);
        let bin_width_hz = spectrum.bin_width_hz();

        let bands = definitions
            .into_iter()
            .map(|definition| {
                let power = band_power(
                    spectrum.magnitudes(),
                    bin_width_hz,
                    definition.lower_frequency_hz,
                    definition.upper_frequency_hz.min(nyquist_hz),
                );
                MusicalBand::new(
                    definition.note_number,
                    definition.lower_frequency_hz,
                    definition.center_frequency_hz,
                    definition.upper_frequency_hz,
                    (power / bin_width_hz).sqrt(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        MusicalSpectrumFrame::new(
            spectrum.sequence(),
            spectrum.position_frames(),
            spectrum.sample_rate(),
            self.tuning_reference_hz,
            bands,
        )
        .map_err(MusicalSpectrumError::InvalidFrame)
    }

    fn band_definitions(&self, nyquist_hz: f32) -> Vec<BandDefinition> {
        let mut definitions = Vec::new();
        let mut note_number = FIRST_NOTE_NUMBER;
        loop {
            let center_frequency_hz = self.frequency_for_note(note_number);
            if center_frequency_hz >= nyquist_hz {
                break;
            }
            definitions.push(BandDefinition {
                note_number,
                lower_frequency_hz: self.boundary_for_note(note_number),
                center_frequency_hz,
                upper_frequency_hz: self.boundary_for_note(note_number + 1),
            });
            let Some(next) = note_number.checked_add(1) else {
                break;
            };
            note_number = next;
        }
        definitions
    }

    fn frequency_for_note(&self, note_number: i16) -> f32 {
        self.tuning_reference_hz
            * 2_f32.powf((f32::from(note_number) - A4_NOTE_NUMBER) / NOTES_PER_OCTAVE)
    }

    fn boundary_for_note(&self, note_number: i16) -> f32 {
        self.tuning_reference_hz
            * 2_f32.powf((f32::from(note_number) - A4_NOTE_NUMBER - 0.5) / NOTES_PER_OCTAVE)
    }
}

/// Integrates a linearly interpolated power spectrum over one pitch band.
/// Coarse low-frequency bins can therefore contribute across a semitone
/// boundary instead of being assigned wholesale to one neighboring pitch.
fn band_power(magnitudes: &[f32], bin_width_hz: f32, lower_hz: f32, upper_hz: f32) -> f32 {
    if upper_hz <= lower_hz || magnitudes.len() < 2 {
        return 0.0;
    }
    let first_segment = (lower_hz / bin_width_hz).floor().max(0.0) as usize;
    let last_segment = (upper_hz / bin_width_hz).ceil() as usize;
    let mut integral = 0.0;

    for index in first_segment..last_segment.min(magnitudes.len() - 1) {
        let segment_lower = index as f32 * bin_width_hz;
        let segment_upper = segment_lower + bin_width_hz;
        let overlap_lower = lower_hz.max(segment_lower);
        let overlap_upper = upper_hz.min(segment_upper);
        if overlap_upper <= overlap_lower {
            continue;
        }
        let left_power = magnitudes[index] * magnitudes[index];
        let right_power = magnitudes[index + 1] * magnitudes[index + 1];
        let power_at = |frequency_hz: f32| {
            let fraction = (frequency_hz - segment_lower) / bin_width_hz;
            left_power + (right_power - left_power) * fraction
        };
        integral += (power_at(overlap_lower) + power_at(overlap_upper))
            * 0.5
            * (overlap_upper - overlap_lower);
    }
    integral
}

struct BandDefinition {
    note_number: i16,
    lower_frequency_hz: f32,
    center_frequency_hz: f32,
    upper_frequency_hz: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MusicalSpectrumError {
    InvalidTuningReference,
    InvalidFrame(MusicalSpectrumFrameError),
}

impl fmt::Display for MusicalSpectrumError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTuningReference => {
                formatter.write_str("tuning reference must be finite and greater than zero")
            },
            Self::InvalidFrame(error) => write!(formatter, "invalid musical spectrum: {error}"),
        }
    }
}

impl Error for MusicalSpectrumError {}

impl From<MusicalSpectrumFrameError> for MusicalSpectrumError {
    fn from(error: MusicalSpectrumFrameError) -> Self {
        Self::InvalidFrame(error)
    }
}
