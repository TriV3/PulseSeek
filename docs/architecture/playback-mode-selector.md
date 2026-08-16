# Playback mode selector

Playback mode selection uses the typed `set_playback_mode` Tauri command. Rust
validates the mode and returns the confirmed value; React rolls back optimistic
selection when the command fails. Supported values are one-shot, loop-current,
sequential, and random.

The selector configures end-of-file behavior already modeled by the playback
domain and worker. In sequential mode, natural completion advances through the
current visible, playable, sorted file list. Folders, filtered-out entries, and
entries removed before completion are never selected. Completion at the last
visible item stops normally, and explicit Stop never advances playback.
In random mode, natural completion selects from the current visible playable
list. The current file is excluded when alternatives exist, so immediate
repeats do not occur. With one playable file, that file may be selected again.
Randomness is injected into the transport for deterministic tests; weighted and
smart shuffle are not supported.

The current desktop bootstrap still uses `FakePlaybackService`, as established
by the pre-existing Tauri bootstrap. The command contract returns the mode
confirmed by its service; wiring the native playback worker into application
bootstrap is tracked by PR-049-2.
