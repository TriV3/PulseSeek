# Musical spectrum

PR-085 implements the pitch-oriented musical spectrum required by FR-VS-007. Frequency energy is
grouped in Rust after the existing FFT worker and rendered as equal-width musical bands on a Canvas
2D surface. No grouping, serialization, or drawing runs on the audio callback.

## Pitch and energy contract

Bands use twelve-tone equal temperament. The application uses A4 = 440 Hz; the analyzer accepts an
explicit positive tuning reference so its mapping is deterministic and testable. Note numbers use
the MIDI numbering convention, extended above 127 when Nyquist permits it. The first rendered band
is C0 (note 12), and the last band is the highest pitch centre below Nyquist.

Each band spans the geometric half-semitone boundaries around its centre. Adjacent boundaries are
contiguous. FFT power is linearly interpolated between neighboring bin centres and integrated over
each band, then converted back to a display magnitude. Interpolation prevents a coarse bass bin
from being assigned wholly to one side of a pitch boundary. It cannot create frequency resolution
that is absent from the 4,096-frame stereo FFT, so closely spaced low notes remain less selective
than midrange notes.

## Bounded data flow

```text
audio callback
  -> bounded lock-free visualization frames
  -> dedicated FFT worker
  -> bounded spectrum queue
  -> musical-band grouping on the spectrum reporter thread
  -> visualization:musical-spectrum event
  -> imperative Canvas 2D renderer
```

The native musical event is produced only while a musical-spectrum subscriber exists. A delivery
gate permits one unacknowledged frame at a time. New work is skipped when the webview has not
acknowledged the prior frame, preserving playback priority and bounded memory. The payload contains
playback-relative metadata, the tuning reference, pitch boundaries, and magnitudes; it contains no
source path or time-domain audio samples.

## Rendering and interaction

The selector mounts exactly one of Waveform, Log analyzer, Linear analyzer, or Musical spectrum.
Switching only changes the frontend subscription and does not issue a playback command or restart
the native pipeline. Musical bands have equal horizontal width, use octave guides, and share the
-90 dB to 0 dB display range used by the other analyzers.

Spectrum frames are held in a ref and drawn outside React rendering. ResizeObserver updates the
backing Canvas, and theme changes repaint the current frame. Colors come exclusively from semantic
analyzer tokens in Light, Dark, Midnight Blue, and High Contrast. The existing seek overlay remains
keyboard and pointer accessible over the Canvas.

## Failure behavior and exclusions

Malformed or unknown-version events are rejected at the TypeScript boundary. Subscription,
analysis, serialization, or drawing failure leaves playback and seek behavior available.

Key detection, chord detection, a tuning-reference setting, persisted visualization selection,
quality controls, stopping the FFT worker when visualizations are disabled, third-party visualizer
plugins, and WebGL rendering are outside PR-085.
