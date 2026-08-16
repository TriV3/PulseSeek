# ADR 0010: Worker-Primed Gapless Sequential Playback

- Status: Accepted
- Date: 2026-08-13

## Context

Sequential playback originally waited for `playback:completed`, then React
started next file. Stopping the current worker and opening a new output stream
created an audible gap, which is especially harmful for mixes split into files.

## Decision

When `sequential` mode and `gapless_playback` preference are enabled:

1. React requests preparation for next visible playable path.
2. Native service opens decoder and configures worker-side resampling.
3. Playback worker primes bounded PCM data before current decoder reaches EOF.
4. Primed PCM is appended to current SPSC ring buffer.
5. Existing output stream consumes boundary continuously.
6. Worker emits track-change metadata through non-audio event path.

Preparation, decoding, resampling, metadata, and event emission stay outside
audio callback. Callback performs only bounded lock-free consumption and signal
mapping.

## Alternatives rejected

- React-driven next-track playback: introduces command, decoder, and stream
  startup latency at every boundary.
- A second output stream per track: platform synchronization and clock drift
  risks; more complex device lifecycle.
- Crossfading: changes source mix timing and does not represent true gapless
  playback.

## Consequences

- Mix segments can continue without application-created silence.
- Worker uses bounded additional memory for primed PCM.
- Decoder failures require candidate skipping and remain observable as UI errors
  only when no candidate can continue.
- Source-file padding, codec delay, and device reconfiguration remain outside
  application control.
