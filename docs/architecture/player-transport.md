# Player transport

Player transport controls stay in React and call typed Tauri playback commands:
play/pause, stop, seek, and volume. Position and duration come from throttled
Rust position events. Previous and next operate on the currently visible
playable-file list; they select and play adjacent entries without creating a
global playback queue.

Controls remain keyboard accessible through native buttons, range inputs, and
labels. Seek is disabled when duration is unavailable. Command failures remain
visible in an alert and do not discard the current file selection. Audio work,
filesystem work, and database work remain outside React and the audio callback.
The Tauri bootstrap still uses a fake playback service in the current phase;
real decoder-to-cpal playback wiring is explicitly tracked by PR-049-2.
