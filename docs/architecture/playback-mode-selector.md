# Playback mode selector

Playback mode selection uses the typed `set_playback_mode` Tauri command. Rust
validates the mode and returns the confirmed value; React rolls back optimistic
selection when the command fails. Supported values are one-shot, loop-current,
sequential, and random.

The selector configures end-of-file behavior already modeled by the playback
domain and worker. Automatic sequential and random file selection remain
separate follow-up behavior.

The current desktop bootstrap still uses `FakePlaybackService`, as established
by the pre-existing Tauri bootstrap. The command contract returns the mode
confirmed by its service; wiring the native playback worker into application
bootstrap is tracked by PR-049-2.
