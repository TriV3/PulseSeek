# A–B repeat (play loop region)

A–B repeat loops the audio between two validated points on the current file
(FR-AU-009). The region is defined by PR-087 (`LoopRegion`) and played by
PR-088 (`set_loop_region` / `clear_loop_region`).

## Region model

A region is the half-open interval `[A, B)`: the start is included, the end is
excluded. Playback reaching B wraps to A and repeats; B itself is never played.
Each cycle resets the consumer position clock to A, so the reported position
returns to A at every wrap.

Regions are validated by `LoopRegion::new` against the current file duration
before they can reach the audio engine. Reversed, equal, and out-of-bounds
points are rejected with a typed `InvalidInput` boundary error
(`playback.control`). A region cannot be set when the duration is unknown.

## Command contract

- `set_loop_region { start_ms, end_ms }` — validates the region, activates it
  on the worker, seeks playback to A, and returns the confirmed start position.
- `clear_loop_region {}` — deactivates the region and leaves the position
  unchanged.

Both commands flow through the typed command envelope
(`SetLoopRegionRequest`/`Response`, `ClearLoopRegionRequest`/`Response`) and
the `PlaybackService` trait. React exposes `setLoopRegion(startMs, endMs)` and
`clearLoopRegion()`; no UI is added in PR-088 (waveform selection is PR-089).

## Playback behavior

- **Wrap at B:** when the engine reaches B it stops producing new frames,
  rewinds the decoder to A, and continues. The wrap is sample-contiguous: the
  ring buffer is never cleared, so no stale frames are audible.
- **Set while playing:** the region takes effect immediately; playback seeks to
  A and loops.
- **Seek inside the region:** the region stays active; playback continues from
  the seek point and keeps looping. The cycle is rebased so the next wrap
  returns to the true A.
- **Seek outside the region** (before A, or at/after B): the region is disabled
  for the current session; playback continues from the seek point under the
  current mode without wrapping. This is a deliberate, documented semantic:
  seeking is treated as an explicit escape from the loop.
- **Clear:** the region is removed; playback continues from the current
  position and follows the current mode's end-of-file behavior (for example,
  OneShot emits `Completed` at file end).
- **Per-file reset:** each file gets a fresh worker, so a region never carries
  over to the next file.

## Short and long regions

- A region shorter than the ring buffer replays from the cached cycle without a
  decoder seek. This is verified with a rejecting-seek fake decoder.
- A region longer than the ring buffer falls back to a decoder seek back to A.
  The prebuffer is large enough that the seek completes before the consumer
  drains the remaining frames.

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