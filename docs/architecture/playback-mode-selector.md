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
Automatic random file selection remains separate follow-up behavior.

The current desktop bootstrap still uses `FakePlaybackService`, as established
by the pre-existing Tauri bootstrap. The command contract returns the mode
confirmed by its service; wiring the native playback worker into application
bootstrap is tracked by PR-049-2.
