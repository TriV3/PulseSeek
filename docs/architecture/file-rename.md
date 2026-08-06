# File Rename

PR-075 (FR-FM-004, FR-FM-009, FR-FM-010). Renames a single selected file
safely and keeps playback, the visible item, and the technical cache
consistent.

## Behavior

- The file list exposes a **Rename** button for the primary selected entry. It
  opens a modal dialog (Enter submits, Escape cancels, focus is trapped and
  restored) prefilled with the current basename.
- The backend validates the new name before touching the filesystem:
  - empty name, `.`/`..`, path separators, NUL, and names over 255 bytes are
    rejected as `InvalidInput`;
  - a target that already exists is rejected as `Conflict` — PulseSeek never
    silently overwrites another file;
  - missing source and permission failures map to `NotFound` and
    `PermissionDenied`.
- On success the file moves within its directory and the response carries
  `old_path`, `new_path`, and `was_playing`. The frontend replaces the entry
  in place (same row, new id and name), moves any session mark to the new id,
  and updates the playing-file id and the persisted last-played path when the
  renamed file was playing.

## Reconcile rules

- **Playback (FR-FM-009):** `PlaybackService::reconcile_path` updates the
  tracked `current_path` when the renamed file is the playing file. On POSIX
  the already-open decoder keeps streaming the original inode, so playback is
  never interrupted; reconciling the tracked path keeps device-switch resume
  and the UI pointing at the new path. On platforms that lock open files
  (Windows), renaming the playing file fails with the mapped permission or
  unavailable error and the dialog keeps the row visible.
- **Cache (FR-FM-010):** the rename service proactively deletes the waveform
  row derived from the old path (raw and canonicalized forms, matching the
  file watcher's candidate logic). A cache failure only logs a warning and
  never blocks the rename. External renames keep working through the file
  watcher's existing invalidation path.

## Failure modes

| Failure | Category | UI result |
| --- | --- | --- |
| Invalid name | `InvalidInput` | Dialog shows the safe message, row kept |
| Target exists | `Conflict` | Dialog shows the safe message, row kept |
| Source missing (external race) | `NotFound` | Dialog shows the safe message, row kept |
| Permission denied | `PermissionDenied` | Dialog shows the safe message, row kept |
| Reconcile playback error | mapped | Command error, rename already applied |
| Cache invalidation failure | none | Warning only; rename succeeds |

The narrow TOCTOU window between the collision check and `fs::rename` on
POSIX (a target created in that window is replaced by the operating system) is
accepted and documented; the frontend's re-enumeration on watcher events
reconciles any resulting view.

## Out of scope

Batch rename, move, copy, rename of manager items, and metadata editing.
