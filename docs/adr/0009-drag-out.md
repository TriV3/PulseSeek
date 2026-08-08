# ADR 0009: File Drag-Out

- Status: Accepted
- Date: 2026-08-07

## Context

PulseSeek must let the user drag files out of the browser into compatible
external applications such as DAWs, editors, and Finder (FR-FM-011). Dragging
a row should carry the whole selection when the row is part of it, otherwise
just that row.

Desktop webviews differ by platform: a standard HTML5 drag with
`text/uri-list` works in Chromium- and WebKitGTK-based webviews, but WKWebView
on macOS cannot deliver file URLs to an external drag session. macOS therefore
needs a native AppKit drag session started from the Rust side.

## Decision

Expose a single `drag_out` command through the versioned command envelope. It
accepts a list of paths and returns a typed result; the UI never receives a
general clipboard or drag-system capability.

The behavior lives behind a domain port `DragOut` with a typed
`DragOutError`. The adapter in `pulseseek-browser-fs` validates that every
path exists before starting a drag, so a missing target is reported as
`NotFound` without initiating a session, and an empty selection is
`InvalidInput`. Platform-specific starting is injected as a `DragStarter` so
the adapter crate stays free of AppKit and Tauri dependencies.

The macOS starter builds one pasteboard item per path carrying a
`public.file-url` and begins an `NSDraggingSession` on the main window's
content view. Starting the AppKit session is queued through Tauri's main-loop
dispatcher so it runs on the next event-loop turn. The
retained `NSURL` pasteboard writers and retained drag source remain valid for
the native session lifetime. The source permits copy and suppresses the return
animation after an accepted drop while preserving it for cancellation; drop
targets receive a reference to the original file and nothing is copied or
written. Platforms without an adapter return `Unsupported`.

In React, non-macOS webviews set `text/uri-list` on the data transfer and let
the HTML5 drag proceed. On macOS, rows are not HTML-draggable: React detects a
primary-button movement beyond a small threshold and invokes `drag_out`
directly. This prevents WKWebView from starting a second drag session and
overwriting the native file-URL pasteboard while Finder or a DAW negotiates
the drop. Platform detection uses Tauri's build-time `TAURI_ENV_PLATFORM`,
with `navigator.platform` and the user agent only as standalone-web fallbacks.

The macOS starter adds three new direct, macOS-only dependencies to
`src-tauri` (`objc2`, `objc2-app-kit`, `objc2-foundation`) from the objc2
family already present transitively in the desktop windowing stack (wry).
The `objc2-app-kit` feature list is scoped to only the classes the starter
uses, and no new transitive crates beyond the existing windowing stack enter
`Cargo.lock`.

## Consequences

- React receives no general clipboard or drag-system capability.
- Dragging carries the whole selection when the dragged row is part of it.
- Missing targets are rejected before a drag session starts; empty selections
  are rejected as invalid input.
- On macOS, drop targets receive references to the original files; nothing is
  copied or written.
- Platform-specific drag behavior has a documented fallback (`text/uri-list`
  on non-macOS webviews, `Unsupported` when no adapter exists).
- Drag cancellation is reported by the platform adapter when available; the
  synchronous command reports that the session started.
