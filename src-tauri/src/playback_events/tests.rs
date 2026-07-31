use std::sync::atomic::Ordering;
use std::time::Duration;

use super::*;

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
