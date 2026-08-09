pub mod types;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::Emitter;

pub use types::*;

pub const CURRENT_EVENT_VERSION: u32 = 1;

pub const EVENT_STATE_CHANGED: &str = "playback:state-changed";
pub const EVENT_POSITION: &str = "playback:position";
pub const EVENT_COMPLETED: &str = "playback:completed";
pub const EVENT_DEVICE_LOST: &str = "audio:device-lost";
pub const EVENT_FOLDER_CHUNK: &str = "browser:folder-chunk";
pub const EVENT_FILE_CHANGE: &str = "browser:file-change";
pub const EVENT_WAVEFORM_READY: &str = "waveform:ready";
pub const EVENT_SPECTRUM_FRAME: &str = "visualization:spectrum";
pub const EVENT_MOVE_PROGRESS: &str = "browser:move-progress";
pub const EVENT_COPY_PROGRESS: &str = "browser:copy-progress";

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmitError;

pub trait PlaybackEventEmitter: Send + Sync {
    fn emit_state(&self, state: &str) -> Result<(), EmitError>;
    fn emit_position(&self, position_ms: u64, duration_ms: Option<u64>) -> Result<(), EmitError>;
    fn emit(&self, event: &str, payload: Value) -> Result<(), EmitError>;
    fn is_disconnected(&self) -> bool;
}

pub struct NoopEventEmitter;

impl PlaybackEventEmitter for NoopEventEmitter {
    fn emit_state(&self, _state: &str) -> Result<(), EmitError> {
        Ok(())
    }

    fn emit_position(&self, _position_ms: u64, _duration_ms: Option<u64>) -> Result<(), EmitError> {
        Ok(())
    }

    fn emit(&self, _event: &str, _payload: Value) -> Result<(), EmitError> {
        Ok(())
    }

    fn is_disconnected(&self) -> bool {
        false
    }
}

pub struct FakeEventEmitter {
    pub events: Mutex<Vec<EventEnvelope>>,
    pub disconnected: AtomicBool,
}

impl FakeEventEmitter {
    pub fn new() -> Self {
        Self { events: Mutex::new(Vec::new()), disconnected: AtomicBool::new(false) }
    }

    pub fn recorded_events(&self) -> Vec<EventEnvelope> {
        self.events.lock().expect("events mutex poisoned").clone()
    }

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
        let payload = types::StateChangedPayload { state: state.to_string() };
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
        let payload = types::PositionPayload { position_ms, duration_ms };
        let envelope = EventEnvelope::new(
            EVENT_POSITION,
            serde_json::to_value(payload).expect("position payload serialization"),
        );
        self.events.lock().expect("events mutex poisoned").push(envelope);
        Ok(())
    }

    fn emit(&self, event: &str, payload: Value) -> Result<(), EmitError> {
        if self.disconnected.load(Ordering::Acquire) {
            return Err(EmitError);
        }
        let envelope = EventEnvelope::new(event, payload);
        self.events.lock().expect("events mutex poisoned").push(envelope);
        Ok(())
    }

    fn is_disconnected(&self) -> bool {
        self.disconnected.load(Ordering::Acquire)
    }
}

pub struct ThrottledEventEmitter {
    inner: Box<dyn PlaybackEventEmitter>,
    min_interval: Duration,
    last_position: Mutex<Instant>,
    dropped_count: AtomicU64,
}

impl ThrottledEventEmitter {
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
            return Ok(());
        }
        *last = now;
        drop(last);
        self.inner.emit_position(position_ms, duration_ms)
    }

    fn emit(&self, event: &str, payload: Value) -> Result<(), EmitError> {
        self.inner.emit(event, payload)
    }

    fn is_disconnected(&self) -> bool {
        self.inner.is_disconnected()
    }
}

pub struct TauriEventEmitter {
    app: tauri::AppHandle,
}

impl TauriEventEmitter {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

impl PlaybackEventEmitter for TauriEventEmitter {
    fn emit_state(&self, state: &str) -> Result<(), EmitError> {
        let payload = StateChangedPayload { state: state.to_string() };
        self.app.emit(EVENT_STATE_CHANGED, &payload).map_err(|_| EmitError)?;
        Ok(())
    }

    fn emit_position(&self, position_ms: u64, duration_ms: Option<u64>) -> Result<(), EmitError> {
        let payload = PositionPayload { position_ms, duration_ms };
        self.app.emit(EVENT_POSITION, &payload).map_err(|_| EmitError)?;
        Ok(())
    }

    fn emit(&self, event: &str, payload: Value) -> Result<(), EmitError> {
        // Emit the raw payload directly. The frontend listener receives this
        // as `event.payload` without an extra EventEnvelope wrapper.
        self.app.emit(event, &payload).map_err(|_| EmitError)?;
        Ok(())
    }

    fn is_disconnected(&self) -> bool {
        // Tauri does not expose a reliable disconnected API.
        false
    }
}

#[cfg(test)]
mod tests;
