use pulseseek_domain::visualization::{
    MusicalBand, MusicalSpectrumFrame, MusicalSpectrumFrameError,
};

fn band(
    note_number: i16,
    lower_frequency_hz: f32,
    center_frequency_hz: f32,
    upper_frequency_hz: f32,
) -> MusicalBand {
    MusicalBand::new(note_number, lower_frequency_hz, center_frequency_hz, upper_frequency_hz, 0.5)
        .expect("valid musical band")
}

#[test]
fn musical_frames_require_consecutive_pitch_bands() {
    let result = MusicalSpectrumFrame::new(
        1,
        0,
        48_000,
        440.0,
        vec![band(69, 427.47, 440.0, 452.89), band(71, 452.89, 466.16, 479.82)],
    );

    assert_eq!(result.unwrap_err(), MusicalSpectrumFrameError::NonContiguousBands);
}

#[test]
fn musical_frames_accept_consecutive_bands_with_shared_boundaries() {
    let result = MusicalSpectrumFrame::new(
        1,
        0,
        48_000,
        440.0,
        vec![band(69, 427.47, 440.0, 452.89), band(70, 452.89, 466.16, 479.82)],
    );

    assert!(result.is_ok());
}
