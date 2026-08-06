# File Move

PR-076 (FR-FM-004, FR-FM-005). Moves one or more selected files into a
chosen target folder with per-file progress, cancellation, and separate
reporting of successful and failed targets. Copy is out of scope (PR-077).

## Behavior

- The file list exposes a **Move…** button enabled when at least one playable
  entry is selected (FR-FM-005). It opens a modal dialog (Enter submits,
  Escape cancels, focus is trapped and restored) with a native folder-picker
  button for the target directory.
- The backend validates the target before starting: a missing or
  non-directory target is rejected as `InvalidInput`/`NotFound` and no file is
  touched.
- `start_move_files` returns a `session_id`; a dedicated worker thread
  (`pulseseek-file-move`) processes the batch and emits `browser:move-progress`
  events, one per file plus a final `done` event. Each event carries
  `session_id`, `completed`, `total`, and per-file results so the UI can render
  progress and a live summary.
- Per-file results report `old_path` and, on success, `new_path`, plus `ok`,
  `category`, `message`, and `diagnostic_code` on failure. Successful and
  failed targets are reported separately (PR-076 acceptance): moved rows
  leave the view, failed rows stay visible with their safe message.
- `cancel_move_files(session_id)` sets a cancellation flag checked between
  files. Remaining files report `Cancelled`, the batch still emits its final
  `done` event, and the summary stays accurate. Cancelling an unknown or
  already-finished session is an idempotent no-op that still succeeds.

## Move semantics

- Same-directory move is a no-op success (source equals target).
- Within one volume, files move with `fs::rename` and keep their inode.
- `CrossesDevices` (move to another volume) falls back to copy-then-delete.
  A copy interrupted midway is cleaned up so no orphaned partial target
  remains.
- A target that already exists is a per-file `Conflict` — PulseSeek never
  silently overwrites another file. The rest of the batch continues.
- Missing source and permission failures map to `NotFound` and
  `PermissionDenied`; a directory passed as source is rejected.

## Reconcile rules

- **Playback:** after each successful move the worker calls
  `PlaybackService::reconcile_path` so the tracked `current_path` follows the
  file. A moved playing file keeps resume/device-switch correctness, matching
  rename (FR-FM-009). A reconcile failure is mapped into the per-file result
  without corrupting it.
- **Cache:** the move service proactively invalidates waveform rows derived
  from each moved-away old path (raw and canonicalized candidates, matching
  the file watcher's logic, mirroring rename FR-FM-010). Cache failures only
  log a warning and never block or fail the move.
- **View:** the frontend removes moved entries from the current folder view,
  clears the selection of moved ids, moves session marks to the new ids, and
  updates the persisted last-played path when the moved file was playing.

## Failure modes

| Failure | Category | UI result |
| --- | --- | --- |
| Invalid/missing target directory | `InvalidInput`/`NotFound` | Dialog error, nothing moved |
| Target exists for a file | `Conflict` | File listed as failed, batch continues |
| Source missing (external race) | `NotFound` | File listed as failed |
| Permission denied | `PermissionDenied` | File listed as failed |
| Cancelled | `Cancelled` | Remaining files listed as cancelled, summary shown |
| Reconcile playback error | mapped | Per-file result, move already applied |
| Cache invalidation failure | none | Warning only; move succeeds |

## Privacy

Errors exposed to the UI contain safe messages and `diagnostic_code`s, never
private absolute paths by default. Structured logs use `tracing` and never log
audio content or full paths unless the diagnostic level requires them.

## Out of scope

Copy (PR-077), move or copy of manager database items, metadata editing, and
undo.
