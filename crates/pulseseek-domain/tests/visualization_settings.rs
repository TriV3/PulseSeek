use pulseseek_domain::visualization::{
    VisualizationMode, VisualizationQuality, VisualizationSettings,
};

#[test]
fn visualization_settings_have_safe_defaults() {
    assert_eq!(
        VisualizationSettings::default(),
        VisualizationSettings::new(
            true,
            VisualizationMode::Waveform,
            VisualizationQuality::Balanced,
        )
    );
}

#[test]
fn visualization_identifiers_round_trip_and_unknown_values_fall_back() {
    for mode in [
        VisualizationMode::Waveform,
        VisualizationMode::Logarithmic,
        VisualizationMode::Linear,
        VisualizationMode::Musical,
    ] {
        assert_eq!(VisualizationMode::from_id(mode.id()), Some(mode));
    }
    for quality in
        [VisualizationQuality::Low, VisualizationQuality::Balanced, VisualizationQuality::High]
    {
        assert_eq!(VisualizationQuality::from_id(quality.id()), Some(quality));
    }

    assert_eq!(VisualizationMode::from_id("plugin"), None);
    assert_eq!(VisualizationQuality::from_id("extreme"), None);
}

#[test]
fn quality_defines_bounded_refresh_rates() {
    assert_eq!(VisualizationQuality::Low.target_fps(), 15);
    assert_eq!(VisualizationQuality::Balanced.target_fps(), 30);
    assert_eq!(VisualizationQuality::High.target_fps(), 60);
}
