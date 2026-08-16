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

Worker queues track-change metadata before the prepared audio enters the ring
buffer. The first prepared sample carries a one-shot transition marker. The
audio callback resets its frame clock and acknowledges that marker with an
atomic store; it never locks, allocates, or emits an event. The position
reporter publishes the queued metadata only after observing that
acknowledgement, so track identity changes when output starts consuming the new
track rather than when worker decoding finishes.

Once that first sample enters the ring, the prepared track is committed and
immediately promoted to active decoder ownership. Any primed PCM that did not
fit in the ring moves to active pending storage. This frees the prepared slot
for the following track without allowing a new `prepare_next` request to
replace or truncate the track currently crossing the audio boundary.

Frontend applies path, zero position, and duration from the track-change event
before changing selection, without issuing another `play` command. Waveform and
analyzer seek canvases reset from path identity immediately; they do not wait
for the next waveform extraction result.

The output stream remains authoritative for playback clock. Visualizers and UI
events are observers and cannot delay or own transition.

## Limitations and fallback

- Source files with incompatible channel layouts are not joined by current
  implementation; preparation failure advances to next candidate.
- Sample-rate conversion remains worker-side and must be primed before boundary.
- Device changes rebuild stream and may produce hardware-dependent interruption.
- Gapless cannot remove silence or clicks already present at source-file edges.

## Verification

Playback tests cover worker preparation, partial ring-buffer refills, the exact
consumed transition boundary, sequential transitions, stale manual selection
protection, and mode changes. Frontend tests cover atomic path/position/duration
updates and playhead reset before new waveform data arrives. Manual
verification must use split mix files, short files, mixed sample rates,
filtering during playback, invalid next files, and final-entry completion.
