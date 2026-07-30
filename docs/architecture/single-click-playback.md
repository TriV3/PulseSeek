# Single-click playback

Selecting a playable browser row with one left click starts playback through
the typed `play` command. Keyboard activation with Enter or Space uses the same
path. React owns transient command-result state for this workflow; Rust remains
source of truth for playback. The existing global state event has no entry ID,
so this workflow does not apply it to rows: doing so could assign a delayed
event from an older selection to a newer row.

Each selection receives a local generation. A late command result from an older
selection cannot replace the newer row's loading, playing, or failed state.
The selected row remains visible after command failure and exposes an alert
message. Double-click and autoplay behavior are intentionally not implemented.
