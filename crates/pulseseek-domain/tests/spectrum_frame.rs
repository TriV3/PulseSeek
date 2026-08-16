use pulseseek_domain::visualization::SpectrumFrame;

#[test]
fn spectrum_frame_exposes_read_only_bins_and_frequency_metadata() {
    let magnitudes = vec![0.0, 0.25, 1.0, 0.5, 0.0];
    let frame = SpectrumFrame::new(7, 256, 48_000, 8, magnitudes.clone()).unwrap();

    assert_eq!(frame.sequence(), 7);
    assert_eq!(frame.position_frames(), 256);
    assert_eq!(frame.sample_rate(), 48_000);
    assert_eq!(frame.fft_size(), 8);
    assert_eq!(frame.bin_width_hz(), 6_000.0);
    assert_eq!(frame.bin_frequency_hz(3), Some(18_000.0));
    assert_eq!(frame.bin_frequency_hz(5), None);
    assert_eq!(frame.magnitudes(), magnitudes);
}

#[test]
fn spectrum_frame_rejects_invalid_fft_metadata_and_bins() {
    assert!(SpectrumFrame::new(0, 0, 0, 8, vec![0.0; 5]).is_err());
    assert!(SpectrumFrame::new(0, 0, 48_000, 6, vec![0.0; 4]).is_err());
    assert!(SpectrumFrame::new(0, 0, 48_000, 8, vec![0.0; 4]).is_err());
    assert!(SpectrumFrame::new(0, 0, 48_000, 8, vec![0.0, f32::NAN, 0.0, 0.0, 0.0]).is_err());
    assert!(SpectrumFrame::new(0, 0, 48_000, 8, vec![0.0, -0.1, 0.0, 0.0, 0.0]).is_err());
}
