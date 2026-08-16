# ADR 0001: Tauri Desktop Stack

- Status: Accepted
- Date: 2026-07-25

## Context

PulseSeek requires a cross-platform desktop UI with large virtualized lists,
folder trees, drag and drop, themes, waveform rendering, and rapid iteration.
The audio and domain core must remain reusable outside the UI.

## Decision

Use Tauri 2 with React, strict TypeScript, and Vite. Keep playback, filesystem,
persistence, analysis, and plugin infrastructure in Rust. Communicate through
narrow typed Tauri commands and versioned events.

## Consequences

- The application uses two language ecosystems.
- The system WebView keeps distribution smaller than a bundled-browser runtime.
- UI work can use the mature React ecosystem.
- Rust remains reusable by a DAW bridge, CLI, or other frontends.
- The IPC boundary requires explicit contracts and security review.
