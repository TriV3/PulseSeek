use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Current version of the event envelope protocol.
pub const CURRENT_EVENT_VERSION: u32 = 1;

/// Event names sent to the frontend.
pub const EVENT_STATE_CHANGED: &str = "playback:state-changed";
pub const EVENT_POSITION: &str = "playback:position";

/// Versioned envelope wrapping every event sent to the frontend.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub version: u32,
    pub event: String,
    pub payload: Value,
}

impl EventEnvelope {
    pub fn new(event: &str, payload: Value) -> Self {
        Self { version: CURRENT_EVENT_VERSION, event: event.to_string(), payload }
    }
}

/// Payload for [`EVENT_STATE_CHANGED`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateChangedPayload {
    pub state: String,
}

/// Payload for [`EVENT_POSITION`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PositionPayload {
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
}

/// Error returned when event emission fails (e.g. subscriber disconnected).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmitError;

/// Abstraction over Tauri's event-emission mechanism, enabling unit tests
/// without a running WebView.
pub trait PlaybackEventEmitter: Send + Sync {
    /// Emits a state-changed event. Returns `Err(EmitError)` when the subscriber
    /// is disconnected.
    fn emit_state(&self, state: &str) -> Result<(), EmitError>;

    /// Emits a position event. Returns `Err(EmitError)` when the subscriber is
    /// disconnected.
    fn emit_position(&self, position_ms: u64, duration_ms: Option<u64>) -> Result<(), EmitError>;

    /// Returns `true` when the frontend subscriber is no longer connected.
    fn is_disconnected(&self) -> bool;
}

// ── NoopEventEmitter ──────────────────────────────────────────────────

/// No-op emitter used in tests that do not care about events.
pub struct NoopEventEmitter;

impl PlaybackEventEmitter for NoopEventEmitter {
    fn emit_state(&self, _state: &str) -> Result<(), EmitError> {
        Ok(())
    }
    fn emit_position(&self, _position_ms: u64, _duration_ms: Option<u64>) -> Result<(), EmitError> {
        Ok(())
    }
    fn is_disconnected(&self) -> bool {
        false
    }
}

// ── FakeEventEmitter ──────────────────────────────────────────────────

/// Fake emitter that records every emitted event for test assertions.
pub struct FakeEventEmitter {
    pub events: Mutex<Vec<EventEnvelope>>,
    pub disconnected: AtomicBool,
}

impl FakeEventEmitter {
    pub fn new() -> Self {
        Self { events: Mutex::new(Vec::new()), disconnected: AtomicBool::new(false) }
    }

    /// Returns a copy of all recorded events.
    pub fn recorded_events(&self) -> Vec<EventEnvelope> {
        self.events.lock().expect("events mutex poisoned").clone()
    }

    /// Returns the number of recorded events.
    pub fn event_count(&self) -> usize {
        self.events.lock().expect("events mutex poisoned").len()
    }
}

impl Default for FakeEventEmitter {
    fn default() -> Self {
        Self::new()
    }
}

impl PlaybackEventEmitter for FakeEventEmitter {
    fn emit_state(&self, state: &str) -> Result<(), EmitError> {
        if self.disconnected.load(Ordering::Acquire) {
            return Err(EmitError);
        }
        let payload = StateChangedPayload { state: state.to_string() };
        let envelope = EventEnvelope::new(
            EVENT_STATE_CHANGED,
            serde_json::to_value(payload).expect("state payload serialization"),
        );
        self.events.lock().expect("events mutex poisoned").push(envelope);
        Ok(())
    }

    fn emit_position(&self, position_ms: u64, duration_ms: Option<u64>) -> Result<(), EmitError> {
        if self.disconnected.load(Ordering::Acquire) {
            return Err(EmitError);
        }
        let payload = PositionPayload { position_ms, duration_ms };
        let envelope = EventEnvelope::new(
            EVENT_POSITION,
            serde_json::to_value(payload).expect("position payload serialization"),
        );
        self.events.lock().expect("events mutex poisoned").push(envelope);
        Ok(())
    }

    fn is_disconnected(&self) -> bool {
        self.disconnected.load(Ordering::Acquire)
    }
}

// ── ThrottledEventEmitter ─────────────────────────────────────────────

/// Wraps an inner [`PlaybackEventEmitter`] and throttles position events to
/// prevent saturating the WebView. State events always pass through.
pub struct ThrottledEventEmitter {
    inner: Box<dyn PlaybackEventEmitter>,
    min_interval: Duration,
    last_position: Mutex<Instant>,
    dropped_count: AtomicU64,
}

impl ThrottledEventEmitter {
    /// Creates a new throttled emitter.
    ///
    /// `min_interval_ms` is the minimum number of milliseconds between two
    /// position events (e.g. 250 ms ≈ 4 events/second).
    pub fn new(inner: Box<dyn PlaybackEventEmitter>, min_interval_ms: u64) -> Self {
        let min_interval = Duration::from_millis(min_interval_ms);
        let last_position = Instant::now().checked_sub(min_interval).unwrap_or_else(Instant::now);
        Self {
            inner,
            min_interval,
            last_position: Mutex::new(last_position),
            dropped_count: AtomicU64::new(0),
        }
    }

    /// Number of position events dropped due to throttling.
    pub fn dropped_count(&self) -> u64 {
        self.dropped_count.load(Ordering::Acquire)
    }
}

impl PlaybackEventEmitter for ThrottledEventEmitter {
    fn emit_state(&self, state: &str) -> Result<(), EmitError> {
        self.inner.emit_state(state)
    }

    fn emit_position(&self, position_ms: u64, duration_ms: Option<u64>) -> Result<(), EmitError> {
        let mut last = self.last_position.lock().expect("last_position mutex poisoned");
        let now = Instant::now();
        if now.duration_since(*last) < self.min_interval {
            self.dropped_count.fetch_add(1, Ordering::Release);
            // Return Ok — throttling is silent, not an error.
            return Ok(());
        }
        *last = now;
        drop(last); // release lock before inner call
        self.inner.emit_position(position_ms, duration_ms)
    }

    fn is_disconnected(&self) -> bool {
        self.inner.is_disconnected()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn fake_emitter_records_state_events() {
        let emitter = FakeEventEmitter::new();
        emitter.emit_state("playing").unwrap();
        assert_eq!(emitter.event_count(), 1);
        let events = emitter.recorded_events();
        assert_eq!(events[0].event, EVENT_STATE_CHANGED);
        let payload: StateChangedPayload =
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
        let payload: PositionPayload = serde_json::from_value(events[0].payload.clone()).unwrap();
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
        let throttled = ThrottledEventEmitter::new(Box::new(inner), 1000); // 1s interval

        // First emission passes.
        throttled.emit_position(1000, None).unwrap();
        assert_eq!(throttled.dropped_count(), 0);

        // Immediate second emission within 1s is dropped.
        throttled.emit_position(2000, None).unwrap();
        assert_eq!(throttled.dropped_count(), 1, "rapid position event should be throttled");

        // State events are never throttled.
        throttled.emit_state("playing").unwrap();
        assert_eq!(throttled.dropped_count(), 1, "state events should not be throttled");
    }

    #[test]
    fn throttled_emitter_passes_non_throttled_events() {
        let inner = FakeEventEmitter::new();
        let throttled = ThrottledEventEmitter::new(Box::new(inner), 50);

        throttled.emit_position(1000, None).unwrap();
        assert_eq!(throttled.dropped_count(), 0);

        // Wait for interval to pass.
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
}
