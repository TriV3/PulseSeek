# File Drag-In

PR-092 (FR-DI-001). Drops external files from the operating system
(Finder/Explorer) onto the PulseSeek window: a dropped audio file is played
while its folder is revealed in the browser, and a dropped folder is revealed
without playing anything.

## Behavior

- The whole window accepts the drop. While files hover, a full-window overlay
  reads **"Drop files to play or reveal"**; it is purely decorative
  (`pointer-events: none`, `aria-hidden`) and never blocks interaction.
- Tauri delivers the drop through `getCurrentWebview().onDragDropEvent`
  (`tauri://drag-drop`) with **absolute filesystem paths**, which the webview
  HTML5 API cannot provide.
- Every dropped path is classified by the backend through a narrow
  `probe_path` command returning one of:
  - `directory` — an existing folder;
  - `playable` — an existing file the decoder can read;
  - `unsupported` — an existing file with a non-audio extension or a corrupt
    stream;
  - `missing` — the path does not exist (a normal drop outcome, not an error).
- Priority when the drop mixes folders and files: **a dropped folder wins**.
  The browser reveals it and nothing is played. Otherwise the **first playable
  audio file** is played and its parent folder is revealed in the browser.
- Non-audio and missing targets are ignored silently. A probe inspection
  failure (for example a permission denial) degrades to "ignore" so a drop
  never interrupts the app.
- The drop behaves like a normal selection: `last_played_file_path`,
  `selected_folder_path`, position, and the parent folder in **Recent
  folders** are persisted and restored on the next launch.

## Architecture

Ports-and-adapters, mirroring the drag-out feature:

- **Domain port** `crates/pulseseek-domain/src/browser/probe`: `ProbeFile`
  trait, `ProbeResult { Directory, Playable, Unsupported, Missing }`, and
  `ProbeError`. `NotFound` is *not* an error — it maps to `Missing` — while
  permission denials map to `PermissionDenied` and anything else to
  `Unavailable`. The domain crate stays dependency-free.
- **Filesystem adapter** `crates/pulseseek-browser-fs/src/probe`:
  `NativeProbe` reads `std::fs::metadata`, classifies directories, reuses the
  browser allow-list (`likely_supported_audio`) so the dropped-file check
  cannot drift from the file list, then probes the stream header with
  `probe_stream_metadata`.
- **Application service** `src-tauri/src/probe_service`: `ProbeService`
  trait, `GenericNativeProbeService<T: ProbeFile>` adapter, and a
  serializable `ProbeKind` enum (the domain `ProbeResult` stays serde-free).
- **Envelope + handler**: `probe_path` request/response types, routed in
  `command_handlers/browsing.rs` and dispatched by `command_envelope`.
- **Frontend**:
  - `src/api/commandEnvelope.ts` — `probePath(path)` typed wrapper;
  - `src/hooks/useFileDrop.ts` — subscribes `onDragDropEvent`, exposes an
    `active` hover state, and suppresses the webview default
    `dragover`/`drop` navigation;
  - `src/components/DropOverlay` — semantic-token overlay;
  - `src/App.tsx` — classifies paths, then calls the existing
    `reopenFolder`/`selectAndRemember` flows so persistence and restore
    behave exactly like a manual selection.

## Privacy

The backend is the source of truth for filesystem checks; React never inspects
dropped paths itself. Probe errors expose only safe messages and
`diagnostic_code`s. `tracing` logs never include dropped paths or audio
content by default.

## Failure modes

| Failure | Category | UI result |
| --- | --- | --- |
| Dropped file missing (race) | — | `Missing`, ignored |
| Dropped file non-audio / corrupt | — | `Unsupported`, ignored |
| Probe permission denied | `PermissionDenied` | treated as unsupported, ignored |
| Webview drop API unavailable | — | overlay absent, browsing/playback unaffected |
| Folder reveal enumeration fails | `BrowserRead` | tree shows existing fallback path |

## Known limitation

On macOS the drag-out gesture for a file row also triggers the drop overlay
until the session ends. It is cosmetic and documented; the drop itself is
ignored because the paths match no new classification.

## Out of scope

Manager database import (a dropped file is browsed/played, never imported),
multi-file queuing, playlist construction, and file-open-at-launch handling.
