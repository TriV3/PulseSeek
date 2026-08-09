# Logarithmic frequency analyzer

PR-083 implements the built-in real-time logarithmic frequency analyzer required by FR-VS-005.
It extends the bounded visualization-frame contract and FFT worker without moving analysis,
serialization, or rendering onto the audio callback.

## Data flow

```text
audio callback
  -> bounded lock-free visualization frames
  -> dedicated FFT worker
  -> bounded spectrum queue
  -> dedicated spectrum event reporter
  -> visualization:spectrum event
  -> requestAnimationFrame Canvas 2D renderer
```

The callback only copies post-volume samples into fixed storage and attempts a non-blocking
publication. FFT calculation and JSON serialization happen on dedicated threads. React does not
store spectrum frames in component state: the event listener replaces one frame reference and
schedules at most one browser animation frame.

## Event contract

`visualization:spectrum` carries format version 1 with:

- `sequence` and `position_frames` for ordering and playback alignment;
- `sample_rate` and `fft_size` for exact frequency-bin mapping;
- a validated one-sided `magnitudes` array containing `fft_size / 2 + 1` finite, non-negative
  values.

The frontend rejects unknown versions, invalid metadata, non-power-of-two FFT sizes, incorrect bin
counts, negative values, and non-finite values. Spectrum events do not contain source file paths or
audio samples.

## Backpressure and lifecycle

Every boundary is bounded. The FFT worker collapses queued time-domain frames to the newest input
and drops output when its spectrum queue is full. The event reporter drains queued spectra before
emitting, and the Canvas listener replaces a pending frame before paint. Slow event delivery or
rendering therefore reduces visualization freshness rather than delaying playback.

Starting a playback stream creates one visualization pipeline. Stop, track replacement, output
device replacement, and service destruction stop its reporter and FFT worker. If the pipeline
cannot start or encounters an unsupported channel layout, playback continues without the analyzer.

## Rendering

Canvas 2D maps 10 Hz to the left edge and Nyquist to the right edge using a logarithmic axis. A
zero-Hertz lower bound is impossible on a logarithmic scale; DC remains present in the FFT contract
but is not plotted on that axis. Stereo output uses a 4,096-frame FFT (about 11.7 Hz per bin at
48 kHz), providing materially more samples below 100 Hz than the earlier 1,024-frame integration.
Coincident high-frequency bins are reduced per display pixel and adjacent points are joined with
quadratic curves, avoiding an angular polyline without adding temporal lag. Magnitude uses a
clamped -90 dB to 0 dB vertical range. ResizeObserver updates the backing canvas dimensions and
repaints without restarting playback or changing FFT work.

The visualization selector displays exactly one surface in the player workspace. Waveform is the
default; selecting Log analyzer unmounts the waveform surface and subscribes the full workspace
Canvas to spectrum events. Switching back unsubscribes the analyzer and resumes waveform loading.
The same imperative position-event overlay remains active over either surface, so current progress,
hover feedback, pointer dragging, and keyboard seeking behave identically in both modes. The
selection is session-only until the visualization-settings work adds persistence.

All analyzer colors come from semantic tokens defined independently by the Light, Dark, Midnight
Blue, and High Contrast themes. A resolved-theme change repaints the current frame. The component
also exposes a disabled contract that unsubscribes, cancels pending paint, and clears the surface;
the persisted preference and worker-load switch remain the responsibility of PR-086.

## Explicit exclusions

Linear frequency mapping, pitch-oriented musical bands, visualization selection persistence and
quality controls, third-party visualizers, and WebGL rendering are outside PR-083.
