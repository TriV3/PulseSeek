# A–B repeat (play loop region)

A–B repeat loops the audio between two validated points on the current file
(FR-AU-009). The region is defined by PR-087 (`LoopRegion`), played by PR-088
(`set_loop_region` / `clear_loop_region`), and selected on the waveform by
PR-089 (Set A / Set B at the playhead).

## Region model

A region is the half-open interval `[A, B)`: the start is included, the end is
excluded. Playback reaching B wraps to A and repeats; B itself is never played.
Each cycle resets the consumer position clock to A, so the reported position
returns to A at every wrap.

Regions are validated by `LoopRegion::new` against the current file duration
before they can reach the audio engine. Reversed, equal, and out-of-bounds
points are rejected with a typed `InvalidInput` boundary error
(`playback.control`). A region cannot be set when the duration is unknown. The
frontend mirrors this policy: placing B at or before A is rejected and the
previous points are kept.

## Command contract

- `set_loop_region { start_ms, end_ms }` — validates the region, activates it
  on the worker, seeks playback to A, and returns the confirmed start position.
- `clear_loop_region {}` — deactivates the region and leaves the position
  unchanged.

Both commands flow through the typed command envelope
(`SetLoopRegionRequest`/`Response`, `ClearLoopRegionRequest`/`Response`) and
the `PlaybackService` trait. React exposes `setLoopRegion(startMs, endMs)` and
`clearLoopRegion()`.

## Waveform selection (PR-089)

The waveform panel shows an A–B cluster with **Set A point**, **Set B point**,
and **Clear A-B** buttons. Each Set button places that point at the position
the waveform canvas currently displays — the exact visual playhead, including
live position events and drag previews — so the recorded point always matches
what the user sees (millisecond precision). Placement is disabled until a file
with a known duration is selected.

- A lone placed point renders as a ghost marker.
- Markers are draggable: grab the bar or the A/B chip to reposition that point
  precisely. The dragged side is clamped so it cannot cross the other
  (`A ≤ B-1`, `B ≥ A+1`); the committed pair is always valid.
- When both points form a valid pair (`A < B`) during an active session,
  `setLoopRegion(A, B)` runs. While stopped, the complete pair remains local
  per-file state and is applied when that file starts again.
- Reversed or equal placement is rejected locally and by `LoopRegion::new`; the
  previous points are kept and an inline notice is shown.
- While stopped, waveform seeking is local and A or B can be placed without an
  active Rust worker.
- **Clear** dismisses the points and the region.
- Shortcuts: `[` places A, `]` places B at the playhead, `a` toggles A–B
  repeat (active → clear, pending valid pair → activate).

Markers and the band are DOM overlays positioned by percentage of the duration
(`--ab-x` / `--ab-width`); they reuse the `wave-selection` theme tokens and
never force a canvas repaint.

## Playback behavior

- **Wrap at B:** when the engine reaches B it stops producing new frames,
  rewinds the decoder to A, and continues. The wrap is sample-contiguous: the
  ring buffer is never cleared, so no stale frames are audible.
- **Set while playing:** the region takes effect immediately; playback seeks to
  A and loops.
- **Stop:** playback and playhead return to zero, but A/B points, duration, and
  region remain visible. Starting the same file creates a fresh worker and
  reapplies the region before continued playback.
- **Seek inside the region:** the region stays active; playback continues from
  the seek point and keeps looping. The cycle is rebased so the next wrap
  returns to the true A.
- **Seek outside the region** (before A, or at/after B): the engine clears the
  region. Playback continues from the seek point under the current mode without
  wrapping. The frontend mirrors this confirmed Rust state by clearing the
  markers on the same seek.
- **Clear:** the region is removed; playback continues from the current
  position and follows the current mode's end-of-file behavior (for example,
  OneShot emits `Completed` at file end).
- **Per-file ownership:** a region never carries over to another file. Stop
  preserves it for the selected file; another selection hides it.

## Short and long regions

- A region shorter than the ring buffer replays from the cached cycle without a
  decoder seek. This is verified with a rejecting-seek fake decoder.
- A region longer than the ring buffer falls back to a decoder seek back to A.
  The prebuffer is large enough that the seek completes before the consumer
  drains the remaining frames. The playback clock resets only when the first
  audible frame at A reaches the callback, never when worker read-ahead seeks.

## Failure modes

- Unknown duration or no active session: `set_loop_region` fails with
  `InvalidInput` before touching the engine.
- Worker unavailable: the command maps to `Unavailable`.
- The region never emits `Completed`/`Failed` terminal events while active;
  wrapping is silent and playback never advances to another file.

## Test strategy

- Worker tests cover wrap-at-B without terminal events, boundary exactness,
  seek into/out of region, clear, short-region cache replay, long-region seek
  fallback, and per-cycle position resets.
- Command-envelope tests cover dispatch, confirmed start, invalid input, and
  service failure.
- `NativePlaybackService` tests cover validation against the session duration
  and worker availability.
