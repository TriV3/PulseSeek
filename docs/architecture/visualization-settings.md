# Visualization settings

PR-086 adds persistent settings for the built-in waveform, logarithmic analyzer, linear analyzer,
and musical spectrum. Third-party visualizers remain outside this contract.

## Persisted contract

`app-cache.sqlite` schema version 5 owns one `visualization_settings` row containing:

- the selected built-in visualization;
- the user's real-time visualization enabled state;
- a `low`, `balanced`, or `high` quality level.

The levels target 15, 30, and 60 spectrum updates per second respectively. They change the hop
between FFT windows, not the FFT size or frequency-bin meaning. Missing settings use waveform,
enabled, and balanced defaults. Invalid values are rejected at both the SQLite and Tauri
boundaries. If the technical cache cannot start, the same contract remains available through a
session-only in-memory service.

## Runtime behavior

The persisted enabled state is distinct from effective runtime activity. FFT input is active only
when the user enabled real-time visualizations, selected an analyzer, and the operating system does
not request reduced motion. Otherwise the waveform is shown without erasing the selected analyzer.

The Tauri settings command applies the effective state to the active playback service after loading
or saving. A shared atomic control lets the audio callback observe activation and the current hop
without a lock, allocation, log, event, or other I/O. When disabled, the tap discards partial state
and returns before copying samples. The bounded FFT and reporter workers consequently receive no
new frames and remain idle; playback is never restarted.

## Accessibility and failure behavior

The selector, enabled checkbox, and quality selector use native keyboard-accessible controls.
`prefers-reduced-motion: reduce` always falls back to the static waveform and explains the fallback
with a status message. Persistence, worker, or event failures leave waveform playback and seeking
available. Settings contain no source path, audio samples, or manager data.
