# Linear frequency analyzer

PR-084 implements the built-in real-time linear frequency analyzer required by FR-VS-006. It
reuses the bounded spectrum event contract delivered by PR-083 and changes only the Canvas mapping
and player selection surface. No analysis, serialization, or rendering runs on the audio callback.

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
publication. The frontend stores the latest validated spectrum in a ref and schedules at most one
browser animation frame. If several events arrive before paint, the pending frame is replaced by
the newest one. Slow rendering therefore reduces visualization freshness instead of delaying
playback.

## Linear mapping

The horizontal axis spans DC at the left edge to Nyquist at the right edge. FFT bin `i` uses:

```text
frequency_hz = i * sample_rate / fft_size
x = frequency_hz / (sample_rate / 2) * canvas_width
```

Every frequency interval receives equal screen width. This is intentionally different from the
logarithmic analyzer, where low frequencies receive more space. Magnitudes use the same clamped
-90 dB to 0 dB vertical range so switching analyzers changes the frequency mapping rather than the
amplitude interpretation. When several bins land on one display pixel, the renderer retains the
strongest magnitude for that pixel. Canvas dimensions follow `ResizeObserver` without restarting
playback or changing FFT work.

## Selection and seek interaction

The visualization selector offers Waveform, Log analyzer, and Linear analyzer and mounts exactly
one of them in the workspace. Selecting Linear analyzer replaces the waveform or logarithmic
surface; it is not layered beside or on top of either graph. Switching modes only changes the
frontend subscriber and Canvas. It does not issue play, pause, stop, or seek commands and does not
restart the native playback pipeline.

The shared imperative seek overlay remains mounted over the selected analyzer. It always shows the
confirmed playback bar and time, shows a distinct bar and time at the hovered position, and supports
click, pointer drag, Left/Right, Home, and End seeking. Position events update these markers without
causing React renders. Pointer and playback markers move through compositor transforms inside a
paint-contained overlay, avoiding layout recalculation and repaint of the analyzer Canvas while the
mouse moves.

All colors come from semantic analyzer and seek tokens provided by the Light, Dark, Midnight Blue,
and High Contrast themes. A resolved-theme change repaints the latest spectrum. The component also
has a disabled contract that unsubscribes, cancels pending paint, clears its surface, and removes the
seek overlay. Persisted selection and stopping worker load when visualizations are disabled remain
the responsibility of PR-086.

## Privacy and failure behavior

Spectrum events contain bin magnitudes and playback-relative frame metadata, but no file paths or
audio samples. Invalid events are rejected by the existing versioned frontend boundary. If event
subscription or drawing is unavailable, playback continues and the static analyzer surface remains
usable for seek interaction.

## Explicit exclusions

Pitch-oriented musical bands, key detection, visualization persistence, quality controls,
third-party visualizers, and WebGL rendering are outside PR-084.
