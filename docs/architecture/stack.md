# Technical Stack

## Status

Accepted.

## Desktop architecture

PulseSeek is a Tauri 2 desktop application:

```text
React + TypeScript
        |
typed Tauri commands and versioned events
        |
Rust application services
        |
domain ports
        |
filesystem / audio / SQLite / analysis / plugin adapters
```

React owns presentation and interaction. Rust owns business state, playback,
files, persistence, analysis, and plugin infrastructure.

## Frontend

- React
- TypeScript with `strict` enabled
- Vite
- pnpm
- Zustand for lightweight global UI state
- TanStack Virtual for large trees and lists
- TanStack Table for sortable/selectable file and library views
- Tailwind CSS
- Radix UI primitives
- PulseSeek component library and semantic design tokens

The Rust backend remains the source of truth for playback, filesystem, and
manager state. Zustand must not become a second domain model.

## Themes

Initial themes:

- PulseSeek Dark
- PulseSeek Light
- System
- Midnight Blue
- High Contrast

All visual components use semantic tokens such as `surface`, `panel`,
`text-primary`, `accent`, and `waveform-positive`. Themes change instantly and
are stored locally. User-authored themes are deferred until after the first
functional player.

## Rust core

- Stable Rust pinned by `rust-toolchain.toml`
- `cpal` for audio output and device enumeration
- `symphonia` for decoding
- `rubato` for resampling
- `lofty` for metadata
- `rusqlite` with one dedicated worker per database
- `tracing` for structured local diagnostics
- `thiserror` for library and domain errors
- `anyhow` only at suitable executable boundaries

The playback engine is built directly on the lower-level audio components.
`rodio` is not the primary engine because precise seeking, short loops,
visualization taps, and a future effect chain require more control.

## Persistence

Separate SQLite files:

```text
app-cache.sqlite
samples.sqlite
music.sqlite
playlists.sqlite
```

Each database has its own:

- Worker
- Repository adapters
- Migrations
- Schema version
- Backup and recovery behavior

No cross-database SQLite foreign keys are permitted.

## Rendering

- Rust computes multiresolution waveform data.
- Canvas 2D renders the first waveform implementation.
- A renderer interface allows WebGL later.
- Real-time visualizations may use WebGL.
- High-frequency rendering does not run through React state updates.

## Plugins

Two independent systems are planned:

1. PulseSeek hosts VST3 effects and PulseSeek visualizers.
2. PulseSeek provides a VST3 DAW bridge for the Sample Manager.

Audio Unit and CLAP may follow later. Plugin scanning should be isolated from
the main UI process, and safe mode must allow startup with third-party plugins
disabled.

## Supported platforms

Development order:

1. macOS on Apple Silicon
2. Universal macOS release for Apple Silicon and Intel
3. Windows
4. Linux

Platform-specific code stays behind explicit ports from the beginning.

## Size and performance budgets

Initial targets on the reference development machine:

- Cold start below 1 second
- Local selection-to-playback below 100 ms
- Responsive browsing with 100,000 files
- First progressive waveform preview below 250 ms when feasible
- Idle memory below 150 MB
- Installer below 40 MB, excluding optional system runtimes
- No playback interruption during waveform or visualization work

A regression above 10% requires an explanation in the PR.

## Security boundary

- React has no general filesystem or shell access.
- Tauri exposes narrow typed commands.
- Rust validates paths and all boundary inputs.
- Tauri capabilities are minimal per window.
- The application uses a strict Content Security Policy.
- Production UI assets are local.
- No generic command execution API is permitted.
