use pulseseek_domain::analysis::{
    AnalysisBlock, AnalysisConfig, AnalysisError, AudioAnalysisSource, AudioFormat, ChannelLayout,
    InMemoryAnalysisSource, MeasurementPoint, SessionEvent, SessionId, SessionRequest,
    SessionState, SourceId, SourceKind, ANALYSIS_SCHEMA_VERSION,
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
fn source_accepts_complete_session_and_rejects_invalid_order() {
    let request = SessionRequest::new(
        SourceId::new("player"),
        SourceKind::Playback,
        MeasurementPoint::Source,
        AudioFormat::mono(48_000).unwrap(),
    );
    let mut source = InMemoryAnalysisSource::new();
    let mut session = source.start(request.clone()).unwrap();
    assert_eq!(session.state(), SessionState::Running);
    let (_, start_event) = source.start_event(request.clone()).unwrap();
    assert_eq!(session.apply(start_event), Err(AnalysisError::DuplicateStart));
    assert_eq!(session.apply(SessionEvent::Resume), Err(AnalysisError::InvalidEvent));
    assert_eq!(session.apply(SessionEvent::Pause).unwrap(), SessionState::Paused);
    let paused_id = session.id().clone();
    assert_eq!(session.apply(SessionEvent::Resume).unwrap(), SessionState::Running);
    assert_eq!(session.id(), &paused_id);
    let resumed = AnalysisBlock::new(
        SourceId::new("player"),
        session.id().clone(),
        SourceKind::Playback,
        MeasurementPoint::Source,
        AudioFormat::mono(48_000).unwrap(),
        0,
        1,
        0,
        false,
        vec![0.0],
    )
    .unwrap();
    assert_eq!(session.apply(SessionEvent::AudioBlock(resumed)).unwrap(), SessionState::Running);
    assert_eq!(session.next_sample(), 1);
    assert_eq!(session.apply(SessionEvent::Pause).unwrap(), SessionState::Paused);
    assert_eq!(session.next_sample(), 1);
    assert_eq!(
        session.apply(SessionEvent::AudioBlock(
            AnalysisBlock::new(
                SourceId::new("player"),
                session.id().clone(),
                SourceKind::Playback,
                MeasurementPoint::Source,
                AudioFormat::mono(48_000).unwrap(),
                0,
                1,
                0,
                false,
                vec![0.0]
            )
            .unwrap()
        )),
        Err(AnalysisError::InvalidEvent)
    );
    assert_eq!(session.apply(SessionEvent::Resume).unwrap(), SessionState::Running);
    let seam_id = session.id().clone();
    assert_eq!(session.apply(SessionEvent::LoopWrap).unwrap(), SessionState::Running);
    assert_eq!(session.id(), &seam_id);
    let wrapped = AnalysisBlock::new(
        SourceId::new("player"),
        session.id().clone(),
        SourceKind::Playback,
        MeasurementPoint::Source,
        AudioFormat::mono(48_000).unwrap(),
        1,
        1,
        1,
        true,
        vec![0.0],
    )
    .unwrap();
    assert_eq!(session.apply(SessionEvent::AudioBlock(wrapped)).unwrap(), SessionState::Running);
    assert!(session.loop_wrapped());
    assert_eq!(
        session.apply(SessionEvent::Gap { first_sample: 1, frame_count: 2 }).unwrap(),
        SessionState::Incomplete
    );
    assert_eq!(session.next_sample(), 3);
    assert_eq!(session.next_sequence(), 3);
    assert_eq!(session.last_gap(), Some((1, 2)));
    assert_eq!(
        session
            .apply(SessionEvent::AudioBlock(
                AnalysisBlock::new(
                    SourceId::new("player"),
                    session.id().clone(),
                    SourceKind::Playback,
                    MeasurementPoint::Source,
                    AudioFormat::mono(48_000).unwrap(),
                    3,
                    1,
                    3,
                    true,
                    vec![0.0]
                )
                .unwrap()
            ))
            .unwrap(),
        SessionState::Incomplete
    );
    assert_eq!(session.last_gap(), Some((1, 2)));
    assert_eq!(session.apply(SessionEvent::Stop).unwrap(), SessionState::Stopped);
    assert_eq!(session.apply(SessionEvent::Resume), Err(AnalysisError::InvalidEvent));
    assert_eq!(session.apply(SessionEvent::Start(request)), Err(AnalysisError::DuplicateStart));
}

#[test]
fn source_tracks_blocks_and_format_changes() {
    let format = AudioFormat::stereo(48_000).unwrap();
    let request = SessionRequest::new(
        SourceId::new("external"),
        SourceKind::External,
        MeasurementPoint::ExternalApplication,
        format,
    );
    let mut source = InMemoryAnalysisSource::new();
    let mut session = source.start(request).unwrap();
    let block = AnalysisBlock::new(
        SourceId::new("external"),
        session.id().clone(),
        SourceKind::External,
        MeasurementPoint::ExternalApplication,
        format,
        0,
        2,
        0,
        false,
        vec![0.0; 4],
    )
    .unwrap();
    assert_eq!(session.apply(SessionEvent::AudioBlock(block)).unwrap(), SessionState::Running);
    assert_eq!(
        session
            .apply(SessionEvent::AudioBlock(
                AnalysisBlock::new(
                    SourceId::new("external"),
                    session.id().clone(),
                    SourceKind::External,
                    MeasurementPoint::ExternalApplication,
                    format,
                    2,
                    1,
                    1,
                    false,
                    vec![0.0; 2]
                )
                .unwrap()
            ))
            .unwrap(),
        SessionState::Running
    );
    let previous = session.id().clone();
    let next =
        session.apply(SessionEvent::FormatChange(AudioFormat::mono(44_100).unwrap())).unwrap();
    assert_eq!(next, SessionState::Running);
    assert_ne!(session.id(), &previous);
    let previous = session.id().clone();
    session
        .apply(SessionEvent::SourceChange {
            source_id: SourceId::new("other"),
            measurement_point: MeasurementPoint::Monitor,
        })
        .unwrap();
    assert_ne!(session.id(), &previous);
    assert_eq!(session.last_gap(), None);
    let before_seek = session.id().clone();
    session.apply(SessionEvent::Seek { first_sample: 100 }).unwrap();
    assert_ne!(session.id(), &before_seek);
    assert_eq!(session.next_sample(), 100);
    assert_eq!(session.next_sequence(), 0);
    assert_eq!(session.apply(SessionEvent::Stop).unwrap(), SessionState::Stopped);
    assert_eq!(session.apply(SessionEvent::Stop), Err(AnalysisError::InvalidEvent));
}

#[test]
fn session_rejects_counter_discontinuity_and_clears_gap_on_reset() {
    let format = AudioFormat::mono(48_000).unwrap();
    let request = SessionRequest::new(
        SourceId::new("player"),
        SourceKind::Playback,
        MeasurementPoint::Source,
        format,
    );
    let mut source = InMemoryAnalysisSource::new();
    let mut session = source.start(request).unwrap();
    let first_block = AnalysisBlock::new(
        SourceId::new("player"),
        session.id().clone(),
        SourceKind::Playback,
        MeasurementPoint::Source,
        format,
        0,
        1,
        0,
        false,
        vec![0.0],
    )
    .unwrap();
    session.apply(SessionEvent::AudioBlock(first_block)).unwrap();
    let invalid_block = AnalysisBlock::new(
        SourceId::new("player"),
        session.id().clone(),
        SourceKind::Playback,
        MeasurementPoint::Source,
        format,
        3,
        1,
        1,
        false,
        vec![0.0],
    )
    .unwrap();
    assert_eq!(
        session.apply(SessionEvent::AudioBlock(invalid_block)),
        Err(AnalysisError::CounterDiscontinuity)
    );
    session.apply(SessionEvent::Gap { first_sample: 1, frame_count: 2 }).unwrap();
    assert_eq!(session.last_gap(), Some((1, 2)));
    session.apply(SessionEvent::Seek { first_sample: 10 }).unwrap();
    assert_eq!(session.last_gap(), None);
    assert_eq!(session.next_sample(), 10);
    assert_eq!(session.next_sequence(), 0);
}

#[test]
fn versioned_defaults_round_trip() {
    let defaults = AnalysisConfig::default();
    let encoded = defaults.to_versioned_string();
    let decoded = AnalysisConfig::from_versioned_string(&encoded).unwrap();

    assert_eq!(decoded, defaults);
    assert_eq!(defaults.schema_version(), ANALYSIS_SCHEMA_VERSION);
}
