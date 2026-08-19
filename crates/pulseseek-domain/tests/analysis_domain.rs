use pulseseek_domain::analysis::{
    AnalysisBlock, AnalysisConfig, AnalysisError, AudioFormat, ChannelLayout, MeasurementPoint,
    SessionId, SourceId, SourceKind, ANALYSIS_SCHEMA_VERSION,
};

#[test]
fn accepts_mono_and_stereo_formats() {
    assert_eq!(AudioFormat::mono(48_000).unwrap().channels(), 1);
    assert_eq!(AudioFormat::stereo(48_000).unwrap().channels(), 2);
}

#[test]
fn rejects_unsupported_channel_layouts() {
    assert_eq!(
        AudioFormat::new(48_000, ChannelLayout::MultiChannel(6)).unwrap_err(),
        AnalysisError::UnsupportedChannelLayout { channels: 6 }
    );
}

#[test]
fn analysis_block_carries_required_metadata() {
    let block = AnalysisBlock::new(
        SourceId::new("player"),
        SessionId::new("session-1"),
        SourceKind::Playback,
        MeasurementPoint::Source,
        AudioFormat::stereo(48_000).unwrap(),
        1_024,
        512,
        7,
        false,
        vec![0.0; 1_024],
    )
    .unwrap();

    assert_eq!(block.schema_version(), ANALYSIS_SCHEMA_VERSION);
    assert_eq!(block.source_id(), &SourceId::new("player"));
    assert_eq!(block.session_id(), &SessionId::new("session-1"));
    assert_eq!(block.measurement_point(), MeasurementPoint::Source);
    assert_eq!(block.first_sample(), 1_024);
    assert_eq!(block.frame_count(), 512);
    assert_eq!(block.sequence(), 7);
    assert!(!block.discontinuity());
}

#[test]
fn versioned_defaults_round_trip() {
    let defaults = AnalysisConfig::default();
    let encoded = defaults.to_versioned_string();
    let decoded = AnalysisConfig::from_versioned_string(&encoded).unwrap();

    assert_eq!(decoded, defaults);
    assert_eq!(defaults.schema_version(), ANALYSIS_SCHEMA_VERSION);
}
