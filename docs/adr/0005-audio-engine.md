# ADR 0005: Controlled Rust Audio Engine

- Status: Accepted
- Date: 2026-07-25

## Context

PulseSeek needs precise seeking, seamless short loops, output-device switching,
waveform and visualization taps, resampling, and a future effect chain.

## Decision

Build the playback engine with:

- `cpal` for output
- `symphonia` for decoding
- `rubato` for resampling
- `lofty` for metadata

Do not use `rodio` as the primary engine. Keep decoding and analysis workers
separate from the real-time audio callback.

## Consequences

- PulseSeek controls timing and the signal path.
- Initial implementation is more demanding.
- The engine can support future visualizers and effects.
- Real-time safety rules are mandatory and tested.
