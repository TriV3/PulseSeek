# ADR 0011: File Associations and OS Opened Files

- Status: Accepted
- Date: 2026-08-15

## Context

PulseSeek must be selectable as the default application for audio files so
that double-clicking a compatible file in the operating system opens
PulseSeek and plays it. On macOS this requires (1) declaring the supported
document types in the app bundle so the app appears in Finder's "Open With"
menu and can be chosen as default, and (2) receiving the file paths the OS
passes to the app, both on a cold launch and while the app is already
running.

The product boundary "browsing or playing a file must never import it"
applies: opening a file from the OS must play it without importing it into
any manager.

## Decision

Declare file associations through Tauri's `bundle.fileAssociations`
configuration, which writes `CFBundleDocumentTypes` into the macOS
`Info.plist` with a `Viewer` role. Register only extensions the decoder can
actually play: `wav`, `wave`, `mp3`, `flac`, `aif`, `aiff`, `ogg`, `oga`,
and `m4a`. Raw `.aac` (ADTS) is not registered because Symphonia 0.5 cannot
demux it.

Extend the Symphonia feature set in `pulseseek-decoder-symphonia` with
`ogg`, `vorbis`, `isomp4`, `aac`, and `alac` so Ogg Vorbis, M4A/AAC, and
M4A/ALAC decode. Opus is excluded: Symphonia 0.5.5 has no Opus decoder.

Route OS-opened files through a small managed `OpenedFiles` queue in the
Tauri layer:

- On a cold launch macOS passes document paths as command-line arguments;
  the setup hook seeds the queue with existing files.
- While running, `RunEvent::Opened` (macOS) converts `file://` URLs to
  paths, seeds the queue, and emits the versioned `browser:opened-files`
  event.
- A `opened_audio_files` command drains the queue; the frontend polls it
  after load (cold start) and listens for the event (warm start).

The frontend probes each opened path with the existing capability check and
plays the first compatible file, reusing the external drag-in flow
(`handleDroppedPaths`). No new process-launch capability reaches React; the
queue only ever contains paths the OS already handed to the app.

## Consequences

- PulseSeek appears in Finder "Open With" for the registered extensions and
  can be set as the default player via the OS.
- Double-clicking an associated file opens PulseSeek and starts playback
  without importing the file.
- The decoder surface expands to Ogg Vorbis, M4A/AAC, and M4A/ALAC;
  `Cargo.lock` gains the transitive Symphonia crates for those features.
- Multi-file opens play the first compatible file; the rest are ignored.
- Warm-start delivery relies on `RunEvent::Opened`, which Tauri derives from
  AppKit's `application(_:openURLs:)`. If a future macOS release routes
  Finder document opens exclusively through `application(_:openFiles:)`, a
  follow-up must register an `NSAppleEventManager` handler for
  `kAEOpenDocuments`, which adds `objc2-core-services` as a structural
  dependency.
- Windows and Linux file associations are out of scope for this decision.
