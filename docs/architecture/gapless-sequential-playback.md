# Gapless sequential playback

`Gapless playback` is an optional player preference. It is enabled by default
and applies only to `sequential` mode. `one-shot`, `loop-current`, and `random`
keep their normal end-of-track behavior.

## User behavior

- Sequential playback prepares the next visible playable entry while current
  entry is playing.
- Filtering, sorting, removal, or navigation changes affect the next prepared
  entry.
- Manual selection has priority and starts selected entry immediately.
- Disabling option cancels pending preparation; current entry finishes, then
  normal sequential playback continues.
- A file that cannot be prepared is skipped and next visible candidate is tried.
- Last entry ends normally.

Preference is persisted as `gapless_playback` and defaults to `true` for older
preference files that do not contain field.

## Audio architecture

Preparation runs through Tauri command on native playback service. Decoder and
resampler run on playback worker, never on React or audio callback. Worker
primes bounded PCM data for next decoder, then appends it to same SPSC ring
buffer used by current decoder. Audio callback consumes one continuous stream;
it does not stop/restart stream or communicate with React at track boundary.

Worker publishes track-change metadata outside callback. Frontend updates
selection, duration, and position from this event without issuing another
`play` command.

The output stream remains authoritative for playback clock. Visualizers and UI
events are observers and cannot delay or own transition.

## Limitations and fallback

- Source files with incompatible channel layouts are not joined by current
  implementation; preparation failure advances to next candidate.
- Sample-rate conversion remains worker-side and must be primed before boundary.
- Device changes rebuild stream and may produce hardware-dependent interruption.
- Gapless cannot remove silence or clicks already present at source-file edges.

## Verification

Playback tests cover worker preparation, sequential transitions, stale manual
selection protection, and mode changes. Manual verification must use split mix
files, short files, mixed sample rates, filtering during playback, invalid next
files, and final-entry completion.
