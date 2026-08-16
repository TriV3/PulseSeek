use pulseseek_domain::visualization::{VisualizationFrame, MAX_VISUALIZATION_FRAME_SAMPLES};

#[test]
fn frame_exposes_read_only_samples_and_metadata() {
    let samples = [0.25, -0.25, 0.5, -0.5];
    let frame = VisualizationFrame::new(7, 128, 48_000, 2, &samples).unwrap();

    assert_eq!(frame.sequence(), 7);
    assert_eq!(frame.position_frames(), 128);
    assert_eq!(frame.sample_rate(), 48_000);
    assert_eq!(frame.channels(), 2);
    assert_eq!(frame.samples(), &samples);
}

#[test]
fn frame_rejects_invalid_audio_shape() {
    assert!(VisualizationFrame::new(0, 0, 0, 2, &[0.0, 0.0]).is_err());
    assert!(VisualizationFrame::new(0, 0, 48_000, 0, &[0.0]).is_err());
    assert!(VisualizationFrame::new(0, 0, 48_000, 2, &[0.0]).is_err());
    assert!(VisualizationFrame::new(0, 0, 48_000, 1, &[]).is_err());
}

#[test]
fn frame_rejects_payload_larger_than_fixed_callback_storage() {
    let samples = vec![0.0; MAX_VISUALIZATION_FRAME_SAMPLES + 1];

    assert!(VisualizationFrame::new(0, 0, 48_000, 1, &samples).is_err());
}
