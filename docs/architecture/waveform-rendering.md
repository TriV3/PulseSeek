# Waveform rendering

The waveform overview renders a multiresolution peak envelope for the
currently selected file on a Canvas 2D surface. Rendering stays off the React
render path and off the audio callback.

## Data flow

1. `WaveformPanel` (React) owns the selected file path and the requested
   resolution (`targetPeaks`). The canvas's `ResizeObserver` drives a
   debounced refetch that maps the measured width to a bucket target via
   `defaultTargetPeaksForWidth` (2 buckets per pixel).
2. `useWaveform` registers the selected file's completion listener before it
   invokes the typed `get_waveform` Tauri command, so even a very short exact
   extraction cannot finish before the UI is ready to receive it. Responses
   are validated against the `WaveformLevel` shape and stale responses from a
   previous selection are ignored.
3. Rust resolves the level through `NativeWaveformService`: it validates the
   path, fingerprints the file identity, serves cached levels from the
   technical cache, and extracts and stores on miss. Extraction uses a bounded
   finest display level (8,192 buckets per channel), enough for two buckets per
   CSS pixel on a 4,096-pixel-wide canvas. This preserves the full-file
   overview without generating detail the current renderer cannot display.
   Extraction runs on a blocking worker off the UI thread.
   The first request uses a 64-bucket sampled preview built from at most four
   coarse seeks across the file. Each seek decodes one contiguous block and
   derives several adjacent peaks from it. This keeps seek cost independent
   from the draw resolution and avoids the zero-amplitude result produced by
   reading only one frame after a compressed-audio seek.
   Once displayed, the canvas requests a width-aware refinement while keeping
   the first preview visible. On a cache miss, that refinement is also sampled
   and capped at 2,048 buckets per channel with the same four-seek ceiling, so
   it reaches display resolution without waiting for a full-file scan.
4. On the first cache miss, Rust builds the sampled response before starting
   the exact pyramid on its dedicated cache worker. This avoids competing for
   disk and decoder time during first paint. Selecting another file cancels
   that obsolete scan; a completed pyramid is stored as technical cache data.
   Rust then emits a typed `waveform:ready` event carrying the source path. If
   that source is still selected, `useWaveform` requests the same resolution
   and replaces the visible preview from cache. Stale events for another
   selection are ignored.
   The decoder adapter, waveform domain reducer, and third-party dependencies
   use optimized code in the Cargo development profile as well as release
   builds. This keeps `tauri dev` representative of production waveform
   throughput; the rest of the workspace retains fast unoptimized development
   builds.
5. `WaveformCanvas` draws the envelope with `drawEnvelope` using semantic
   color tokens (`--wave`, `--wave-grid`, `--wave-soft`, `--wave-playhead`).
   A short clipping animation reveals the first available waveform from left
   to right; subsequent width-aware and exact refinements do not replay it.
   Reduced-motion preferences disable the animation.
6. The active renderer style (`solid`, `gradient`, or `outline`) is persisted
   in player preferences and applied through `WaveformStyleSelector` in the
   transport options strip. A style change repaints the canvas only; it never
   re-requests waveform data or regenerates peaks.

## Renderer styles

`drawEnvelope` supports three styles (FR-VS-004):

- **outline** (default): fills the envelope body with the `--wave-soft` token
  and strokes the min/max edges with `--wave`, so the waveform center reads
  clearly against the panel background.
- **solid**: fills the closed area between the min and max edges with the
  `--wave` token.
- **gradient**: fills the same area with a vertical token gradient, strongest
  at the channel center (`--wave`) and fading toward the row edges
  (`--wave-soft`).

Grid lines and the playhead are drawn in every style. All colors come from
semantic tokens; the renderer never hard-codes a theme color.

## Playhead never re-renders React

Playback position events are throttled at the Rust boundary. `WaveformCanvas`
consumes them into a ref and schedules a `requestAnimationFrame` overlay update;
static envelope geometry is cached and never rebuilt for position events. The
position is never stored in React state, so high-frequency updates do not
re-render the component. Hover marker and label updates are coalesced into one
frame. Late frames are dropped rather than delaying audio.

## Seek interaction

`WaveformCanvas` is the seek surface (FR-VS-003):

- Click or drag converts the pointer x coordinate into a millisecond target
  through `positionMsForX`, clamped to `[0, durationMs]`, and forwards it to
  `onSeek`. The app wires that to the validated `seek` command through
  `usePlaybackTransport.handleSeek`, which keeps the confirmed Rust position
  and surfaces command failures in the transport alert.
- Keyboard: `ArrowLeft`/`ArrowRight` step by selected seek-step preference. Auto
  uses duration plateaus: below 15 s → 1 s; 15 s–1 min → 2 s; 1–7 min → 5 s;
  7–15 min → 10 s; 15–30 min → 15 s; 30–60 min → 20 s; above 60 min → 30 s.
  Presets range from 1 s to 30 s. When duration is unavailable, auto uses 5 s.
  The step is aligned
  with the existing keyboard shortcuts), `Home` seeks to zero, `End` to the
  duration.
- During a drag the playhead previews the pointer target imperatively; the next
  confirmed position event reconciles the visual progress with the Rust
  position (PR-036).
- The canvas exposes `role="slider"` with `aria-valuenow` written imperatively
  on every draw, so screen readers follow position without React re-renders.
  Without a known duration the slider is `aria-disabled` and not tabbable.
- Seeking beyond the duration is clamped on the frontend because the backend
  rejects targets outside a known duration (`Duration::seek_to`).

## Failure and degradation behavior

- A cache miss falls back to extraction; a cache that is unavailable or fails
  to write degrades to extract-without-store, never to a playback failure.
- An extraction error is mapped to a typed boundary error and surfaces in the
  panel as an accessible `role="alert"` message.
- While no file is selected the canvas renders empty. The canvas clears itself
  when waveform data is unavailable.
- The cache read/write is best-effort by design; browsing or playing a file
  never depends on the waveform path.

## Testing

Renderer geometry and drawing are pure functions covered by unit tests,
including per-style rendering (outline strokes, solid fill, gradient fill) and
token consumption. `WaveformCanvas` behavior (drawing, playhead, debounced
resize refetch, style change without refetch) is covered with a mocked Canvas
2D context and a triggerable `ResizeObserver`. `WaveformPanel` tests cover
selection, empty state, error state, resolution refetch, and the guarantee that
a style change never refetches waveform data. The e2e mock backend returns a
minimal valid waveform so selecting a file keeps the overview clean;
`waveform-styles.spec.ts` asserts style repaints without new `get_waveform`
calls and compares committed screenshot baselines per platform.
Domain tests also lock the sampled preview to at most four coarse seeks, while
decoder integration tests verify non-zero envelopes for non-silent WAV, FLAC,
and MP3 fixtures.
