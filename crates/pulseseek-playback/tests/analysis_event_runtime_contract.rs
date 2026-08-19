use pulseseek_domain::analysis_events::{EventFamily, EventValidity};
use pulseseek_playback::{AnalysisEventRuntime, EventEnvelope};

fn event(family: EventFamily, sequence: u64) -> EventEnvelope {
    EventEnvelope::new(family, sequence, sequence, EventValidity::Measured).unwrap()
}

#[test]
fn slow_family_does_not_block_unrelated_family() {
    let runtime = AnalysisEventRuntime::new(1);
    let slow = runtime.subscribe(EventFamily::Spectrum, 1).unwrap();
    let fast = runtime.subscribe(EventFamily::Levels, 1).unwrap();
    assert!(runtime.publish(event(EventFamily::Spectrum, 1)).is_accepted());
    assert!(runtime.publish(event(EventFamily::Spectrum, 2)).is_dropped());
    assert!(runtime.publish(event(EventFamily::Levels, 1)).is_accepted());
    assert_eq!(fast.try_receive().unwrap().sequence(), 1);
    drop(slow);
}

#[test]
fn continuous_overflow_marks_only_family_incomplete() {
    let runtime = AnalysisEventRuntime::new(1);
    let continuous = runtime.subscribe(EventFamily::Loudness, 1).unwrap();
    let other = runtime.subscribe(EventFamily::Levels, 1).unwrap();
    runtime.publish(event(EventFamily::Loudness, 1));
    assert!(runtime.publish(event(EventFamily::Loudness, 2)).is_gap());
    assert_eq!(runtime.family_validity(EventFamily::Loudness), EventValidity::Incomplete);
    assert_eq!(runtime.family_validity(EventFamily::Levels), EventValidity::Measured);
    drop(continuous);
    drop(other);
}

#[test]
fn unsubscribe_is_idempotent_and_receiver_drop_disconnects() {
    let runtime = AnalysisEventRuntime::new(2);
    let receiver = runtime.subscribe(EventFamily::Diagnostics, 2).unwrap();
    let id = receiver.id();
    assert!(runtime.unsubscribe(id));
    assert!(!runtime.unsubscribe(id));
    assert!(!runtime.publish(event(EventFamily::Diagnostics, 1)).is_accepted());
}

#[test]
fn receiver_supports_explicit_unsubscribe() {
    let runtime = AnalysisEventRuntime::new(1);
    let receiver = runtime.subscribe(EventFamily::Session, 1).unwrap();
    assert!(receiver.unsubscribe());
    assert!(!runtime.publish(event(EventFamily::Session, 1)).is_accepted());
}

#[test]
fn runtime_unsubscribe_then_receiver_drop_preserves_other_receiver() {
    let runtime = AnalysisEventRuntime::new(2);
    let first = runtime.subscribe(EventFamily::Levels, 2).unwrap();
    let second = runtime.subscribe(EventFamily::Levels, 2).unwrap();
    let first_id = first.id();
    assert!(runtime.unsubscribe(first_id));
    drop(first);
    assert!(runtime.publish(event(EventFamily::Levels, 1)).is_accepted());
    assert_eq!(second.try_receive().unwrap().sequence(), 1);
}

#[test]
fn receiver_unsubscribe_then_drop_preserves_other_receiver() {
    let runtime = AnalysisEventRuntime::new(2);
    let first = runtime.subscribe(EventFamily::Levels, 2).unwrap();
    let second = runtime.subscribe(EventFamily::Levels, 2).unwrap();
    assert!(first.unsubscribe());
    assert!(runtime.publish(event(EventFamily::Levels, 1)).is_accepted());
    assert_eq!(second.try_receive().unwrap().sequence(), 1);
}

#[test]
fn runtime_preserves_order_and_timestamps() {
    let runtime = AnalysisEventRuntime::new(3);
    let receiver = runtime.subscribe(EventFamily::Levels, 3).unwrap();
    runtime.publish(event(EventFamily::Levels, 1));
    runtime
        .publish(EventEnvelope::new(EventFamily::Levels, 2, 4, EventValidity::Measured).unwrap());
    assert_eq!(receiver.try_receive().unwrap().sequence(), 1);
    assert_eq!(receiver.try_receive().unwrap().timestamp_samples(), 4);
}

#[test]
fn cadence_is_enforced_for_cadenced_families() {
    let runtime = AnalysisEventRuntime::new(1);
    let receiver = runtime.subscribe(EventFamily::Spectrum, 1).unwrap();
    assert!(matches!(
        runtime.publish_at(event(EventFamily::Spectrum, 1), 10),
        pulseseek_playback::MeteringPublishResult::CadenceRejected
    ));
    assert!(runtime.publish_at(event(EventFamily::Spectrum, 1), 15).is_accepted());
    drop(receiver);
}

#[test]
fn cadence_policies_are_explicit_for_every_family() {
    for family in EventFamily::ALL {
        assert!(matches!(
            family.policy(),
            pulseseek_domain::analysis_events::DeliveryPolicy::OnChange
                | pulseseek_domain::analysis_events::DeliveryPolicy::LatestOnly
                | pulseseek_domain::analysis_events::DeliveryPolicy::ContinuousAndDisplay
                | pulseseek_domain::analysis_events::DeliveryPolicy::Cadenced { .. }
        ));
    }
}

#[test]
fn unknown_event_schema_is_rejected_before_runtime_delivery() {
    assert_eq!(
        pulseseek_domain::analysis_events::SchemaVersion::new(99),
        Err(pulseseek_domain::analysis_events::EventContractError::UnsupportedSchemaVersion(99))
    );
}
