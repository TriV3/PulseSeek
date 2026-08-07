# File Copy

PR-077 (FR-FM-004, FR-FM-005). Copies one or more selected files into a
chosen target folder with per-file progress, cancellation, and separate
reporting of successful and failed targets. Manager import is out of scope.

## Behavior

- The file list exposes a **Copy…** button enabled when at least one playable
  entry is selected (FR-FM-005). It opens a modal dialog (Enter submits,
  Escape cancels, focus is trapped and restored) with a native folder-picker
  button for the target directory, mirroring the Move dialog.
- The backend validates the target before starting: a missing or
  non-directory target is rejected as `InvalidInput`/`NotFound` and no file is
  touched.
- `start_copy_files` returns a `session_id`; a dedicated worker thread
  (`pulseseek-file-copy`) processes the batch and emits
  `browser:copy-progress` events, one per file plus a final `done` event.
  Each event carries `session_id`, `completed`, `total`, and per-file results
  so the UI can render progress and a live summary.
- Per-file results report the source `path` and, on success, the created
  `new_path`, plus `ok`, `category`, `message`, and `diagnostic_code` on
  failure. Successful and failed targets are reported separately (PR-077
  acceptance). Copied rows stay in the view: copying never modifies or
  removes the original.
- `cancel_copy_files(session_id)` sets a cancellation flag checked between
  files. Remaining files report `Cancelled`, the batch still emits its final
  `done` event, and the summary stays accurate. Cancelling an unknown or
  already-finished session is an idempotent no-op that still succeeds.

## Copy semantics

- Copying is read-only over the source: originals are never modified
  (PR-077 acceptance).
- Files are copied with `std::fs::copy` into the target directory, keeping
  each file's name. Copying works across volumes without a fallback.
- A target that already exists is a per-file `Conflict` — PulseSeek never
  silently overwrites another file. The rest of the batch continues.
- Copying a file into its own directory is reported as a `Conflict` so the
  source stays intact.
- Missing source and permission failures map to `NotFound` and
  `PermissionDenied`; a directory passed as source is rejected as
  `InvalidInput`.
- A failed copy removes the partial target again (best effort) so no
  orphaned half-copied file remains.

## Reconcile rules

- **Playback:** none. The source keeps its path, so the tracked
  `current_path` stays valid.
- **Cache:** none. The source keeps its cached waveform row; the new copy
  simply has no cached row yet. There is nothing to invalidate because no
  path changed.

## Failure modes

| Failure | Category | UI result |
| --- | --- | --- |
| Invalid/missing target directory | `InvalidInput`/`NotFound` | Dialog error, nothing copied |
| Target exists for a file | `Conflict` | File listed as failed, batch continues |
| Source missing (external race) | `NotFound` | File listed as failed |
| Permission denied | `PermissionDenied` | File listed as failed |
| Cancelled | `Cancelled` | Remaining files listed as cancelled, summary shown |

## Privacy

Errors exposed to the UI contain safe messages and `diagnostic_code`s, never
private absolute paths by default. Structured logs use `tracing` and never log
audio content or full paths unless the diagnostic level requires them.

## Out of scope

Manager import, metadata editing, undo, and batch rename.
