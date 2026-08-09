use std::sync::atomic::Ordering;
use std::time::Duration;

use super::*;

#[test]
fn spectrum_delivery_never_queues_more_than_one_unacknowledged_frame() {
    let gate = SpectrumDeliveryGate::new();

    gate.subscribe();
    assert!(gate.try_begin_delivery());
    assert!(!gate.try_begin_delivery());

    gate.acknowledge();
    assert!(gate.try_begin_delivery());
}

#[test]
fn spectrum_delivery_stops_without_a_subscriber_and_resets_on_resubscribe() {
    let gate = SpectrumDeliveryGate::new();

    assert!(!gate.try_begin_delivery());
    gate.subscribe();
    assert!(gate.try_begin_delivery());
    gate.unsubscribe();
    assert!(!gate.try_begin_delivery());

    gate.subscribe();
    assert!(gate.try_begin_delivery());
}

#[test]
fn fake_emitter_records_state_events() {
    let emitter = FakeEventEmitter::new();
    emitter.emit_state("playing").unwrap();
    assert_eq!(emitter.event_count(), 1);
    let events = emitter.recorded_events();
    assert_eq!(events[0].event, EVENT_STATE_CHANGED);
    let payload: types::StateChangedPayload =
        serde_json::from_value(events[0].payload.clone()).unwrap();
    assert_eq!(payload.state, "playing");
}

#[test]
fn fake_emitter_records_position_events() {
    let emitter = FakeEventEmitter::new();
    emitter.emit_position(12345, Some(60000)).unwrap();
    assert_eq!(emitter.event_count(), 1);
    let events = emitter.recorded_events();
    assert_eq!(events[0].event, EVENT_POSITION);
    let payload: types::PositionPayload =
        serde_json::from_value(events[0].payload.clone()).unwrap();
    assert_eq!(payload.position_ms, 12345);
    assert_eq!(payload.duration_ms, Some(60000));
}

#[test]
fn fake_emitter_detects_disconnected_subscriber() {
    let emitter = FakeEventEmitter::new();
    assert!(!emitter.is_disconnected());
    emitter.disconnected.store(true, Ordering::Release);
    assert!(emitter.is_disconnected());
    assert!(emitter.emit_state("playing").is_err());
    assert!(emitter.emit_position(0, None).is_err());
}

#[test]
fn event_envelope_has_correct_version() {
    let envelope = EventEnvelope::new("test", serde_json::json!({"key": "value"}));
    assert_eq!(envelope.version, CURRENT_EVENT_VERSION);
    assert_eq!(envelope.event, "test");
    assert_eq!(envelope.payload, serde_json::json!({"key": "value"}));
}

#[test]
fn spectrum_payload_is_versioned_and_serializable() {
    let payload = types::SpectrumFramePayload {
        format_version: types::SPECTRUM_FORMAT_VERSION,
        sequence: 7,
        position_frames: 2_048,
        sample_rate: 48_000,
        fft_size: 1_024,
        magnitudes: vec![0.0, 0.25, 1.0],
    };

    let envelope = EventEnvelope::new(
        EVENT_SPECTRUM_FRAME,
        serde_json::to_value(&payload).expect("spectrum payload serialization"),
    );

    assert_eq!(envelope.event, "visualization:spectrum");
    assert_eq!(envelope.payload["format_version"], 1);
    assert_eq!(envelope.payload["sequence"], 7);
    assert_eq!(envelope.payload["magnitudes"], serde_json::json!([0.0, 0.25, 1.0]));
}

#[test]
fn musical_spectrum_payload_is_versioned_and_serializable() {
    let payload = types::MusicalSpectrumFramePayload {
        format_version: types::MUSICAL_SPECTRUM_FORMAT_VERSION,
        sequence: 8,
        position_frames: 4_096,
        sample_rate: 48_000,
        tuning_reference_hz: 440.0,
        bands: vec![types::MusicalBandPayload {
            note_number: 69,
            lower_frequency_hz: 427.47,
            center_frequency_hz: 440.0,
            upper_frequency_hz: 452.89,
            magnitude: 0.8,
        }],
    };

    let envelope = EventEnvelope::new(
        EVENT_MUSICAL_SPECTRUM_FRAME,
        serde_json::to_value(&payload).expect("musical spectrum payload serialization"),
    );

    assert_eq!(envelope.event, "visualization:musical-spectrum");
    assert_eq!(envelope.payload["format_version"], 1);
    assert_eq!(envelope.payload["tuning_reference_hz"], 440.0);
    assert_eq!(envelope.payload["bands"][0]["note_number"], 69);
}

#[test]
fn throttled_emitter_tracks_dropped_events() {
    let inner = FakeEventEmitter::new();
    let throttled = ThrottledEventEmitter::new(Box::new(inner), 1000);

    throttled.emit_position(1000, None).unwrap();
    assert_eq!(throttled.dropped_count(), 0);

    throttled.emit_position(2000, None).unwrap();
    assert_eq!(throttled.dropped_count(), 1, "rapid position event should be throttled");

    throttled.emit_state("playing").unwrap();
    assert_eq!(throttled.dropped_count(), 1, "state events should not be throttled");
}

#[test]
fn throttled_emitter_passes_non_throttled_events() {
    let inner = FakeEventEmitter::new();
    let throttled = ThrottledEventEmitter::new(Box::new(inner), 50);

    throttled.emit_position(1000, None).unwrap();
    assert_eq!(throttled.dropped_count(), 0);

    std::thread::sleep(Duration::from_millis(60));

    throttled.emit_position(2000, None).unwrap();
    assert_eq!(throttled.dropped_count(), 0, "position event after interval should pass");
}

#[test]
fn throttled_emitter_detects_disconnect() {
    let inner = FakeEventEmitter::new();
    inner.disconnected.store(true, Ordering::Release);
    let throttled = ThrottledEventEmitter::new(Box::new(inner), 100);

    assert!(throttled.is_disconnected());
    assert!(throttled.emit_state("playing").is_err());
    assert!(throttled.emit_position(0, None).is_err());
}
