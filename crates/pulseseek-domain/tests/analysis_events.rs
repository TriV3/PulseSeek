use pulseseek_domain::analysis::{MeasurementPoint, SessionId, SourceId};
use pulseseek_domain::analysis_events::*;

fn metadata(sequence: u64, timestamp_samples: u64) -> EventMetadata {
    EventMetadata::new(
        SessionId::new("session"),
        SourceId::new("source"),
        MeasurementPoint::Source,
        sequence,
        timestamp_samples,
        EventValidity::Measured,
    )
    .unwrap()
}

#[test]
fn accepts_v1_and_rejects_unknown_schema() {
    assert!(SchemaVersion::new(1).is_ok());
    assert_eq!(SchemaVersion::new(2), Err(EventContractError::UnsupportedSchemaVersion(2)));
}

#[test]
fn event_families_have_independent_delivery_policies() {
    assert_eq!(EventFamily::Spectrum.policy(), DeliveryPolicy::Cadenced { min_hz: 15, max_hz: 60 });
    assert_eq!(EventFamily::Spectrogram.policy(), DeliveryPolicy::LatestOnly);
    assert_eq!(EventFamily::Session.policy(), DeliveryPolicy::OnChange);
    assert_eq!(EventFamily::Loudness.policy(), DeliveryPolicy::ContinuousAndDisplay);
    assert_eq!(
        EventFamily::Diagnostics.policy(),
        DeliveryPolicy::Cadenced { min_hz: 1, max_hz: 5 }
    );
}

#[test]
fn metadata_requires_ordered_values_and_validity() {
    assert!(metadata(1, 10).validate_after(&metadata(0, 9)).is_ok());
    assert_eq!(
        metadata(1, 10).validate_after(&metadata(0, 11)),
        Err(EventContractError::TimestampDecreased)
    );
    assert_eq!(
        metadata(0, 10).validate_after(&metadata(0, 10)),
        Err(EventContractError::SequenceNotIncreasing)
    );
    assert_eq!(
        EventValidity::Invalid { reason: InvalidReason::QueueSaturated }.validity(),
        Validity::Invalid
    );
}

#[test]
fn session_change_resets_ordering_scope() {
    let next_session = EventMetadata::new(
        SessionId::new("next"),
        SourceId::new("source"),
        MeasurementPoint::Source,
        0,
        0,
        EventValidity::Unavailable,
    )
    .unwrap();
    assert!(next_session.validate_after(&metadata(u64::MAX, u64::MAX)).is_ok());
}

#[test]
fn experimental_metadata_exposes_formula_baseline_window_and_algorithm() {
    let value = ExperimentalMetadata::new("correlation", "unity", "hann", "stereo-v1").unwrap();
    assert_eq!(value.formula(), "correlation");
    assert_eq!(value.baseline(), "unity");
    assert_eq!(value.window(), "hann");
    assert_eq!(value.algorithm_version(), "stereo-v1");
}

#[test]
fn all_event_families_are_closed_and_named() {
    let families = EventFamily::ALL;
    assert_eq!(families.len(), 10);
    assert_eq!(EventFamily::Waveform.wire_name(), "metering.waveform");
}
