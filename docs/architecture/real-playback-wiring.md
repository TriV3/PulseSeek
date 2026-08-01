# Real playback wiring

Replaces the Tauri `FakePlaybackService` with `NativePlaybackService` that
connects decoder workers (`pulseseek-decoder-symphonia`), the playback engine
(`pulseseek-playback`), and the native audio output (`CpalAudioOutput`) into
a working real-time audio pipeline.

## Architecture

```
File selection
    ↓ (play command via command envelope)
NativePlaybackService
    ├── DecoderRegistry::open(path)   → Decoder
    ├── PlaybackWorker::start(decoder) → worker + consumer
    ├── CpalAudioOutput::open_stream(consumer, channels)
    └── cpal audio callback reads frames from consumer ring buffer
```

- `NativePlaybackService` implements the existing `PlaybackService` trait.
- `CpalAudioOutput` is shared via `Arc<Mutex<CpalAudioOutput>>` between
  `NativeAudioDeviceService` and `NativePlaybackService`.
- Decoder and worker run on a dedicated non-audio thread.
- Audio callback runs on the platform audio thread; no allocation, locking,
  I/O, or SQL allowed there.

## Device rebind

When the user selects a different output device while playback is active, the
playback service snapshots the current path, clock, and playing/paused state,
opens the replacement hardware, rebuilds the decoder worker for the new output
sample rate, seeks to the captured clock, and restores the previous state. The
old stream is not reported as a user-requested stop, and playback never requires
a second click. A hardware-dependent transition of a few milliseconds may occur
while CoreAudio activates the replacement device.

Device loss pauses the output stream; the next command returns an error so
the frontend can prompt the user to select a working device.

## Test strategy

- `FakeAudioOutput` implements `AudioOutput` for service-level tests.
- Real cpal integration is exercised manually: play WAV/FLAC/MP3,
  pause/resume, seek, volume, device change, unplug/replug.

## Current limitations

- Asynchronous device-loss notifications from the cpal error callback are not
  forwarded to the frontend as Tauri events; the frontend discovers loss on
  the next command call.
- Device IDs are adapter-scoped names and not stable across sessions.
