# Waveform rendering

The waveform overview renders a multiresolution peak envelope for the
currently selected file on a Canvas 2D surface. Rendering stays off the React
render path and off the audio callback.

## Data flow

1. `WaveformPanel` (React) owns the selected file path and the requested
   resolution (`targetPeaks`). The canvas's `ResizeObserver` drives a
   debounced refetch that maps the measured width to a bucket target via
   `defaultTargetPeaksForWidth` (2 buckets per pixel).
2. `useWaveform` fetches the matching resolution level through the typed
   `get_waveform` Tauri command. Responses are validated against the
   `WaveformLevel` shape and stale responses from a previous selection are
   ignored.
3. Rust resolves the level through `NativeWaveformService`: it validates the
   path, fingerprints the file identity, serves cached levels from the
   technical cache, and extracts and stores on miss. Extraction runs on a
   blocking worker off the UI thread.
4. `WaveformCanvas` draws the envelope with `drawEnvelope` using semantic
   color tokens (`--wave`, `--wave-grid`, `--wave-playhead`).

## Playhead never re-renders React

Playback position events are throttled at the Rust boundary. `WaveformCanvas`
consumes them into a ref and schedules a `requestAnimationFrame` redraw; the
position is never stored in React state, so high-frequency updates do not
re-render the component. Late frames are dropped rather than delaying audio.

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

Renderer geometry and drawing are pure functions covered by unit tests.
`WaveformCanvas` behavior (drawing, playhead, debounced resize refetch) is
covered with a mocked Canvas 2D context and a triggerable `ResizeObserver`.
`WaveformPanel` tests cover selection, empty state, error state, and resolution
refetch. The e2e mock backend returns a minimal valid waveform so selecting a
file keeps the overview clean.
