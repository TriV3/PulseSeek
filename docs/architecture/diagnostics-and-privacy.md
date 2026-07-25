# Diagnostics and Privacy

## Logging

PulseSeek uses structured Rust logging through `tracing`.

- Logs rotate and have a bounded total size.
- Log level is configurable.
- Audio content is never logged.
- Private paths are masked in exported reports by default.
- Authentication secrets, tokens, plugin state blobs, and personal metadata are
  never logged.
- React failures are forwarded to local diagnostics without automatic cloud
  submission.

## User diagnostics

The application will provide:

- Open log folder
- Copy application and platform versions
- Export a redacted diagnostic report
- Start in safe mode with third-party plugins disabled
- Reset technical cache without deleting manager databases

## Telemetry

There is no telemetry in the first version.

Any future telemetry must:

- Be disabled by default
- Require explicit consent
- Document every collected field
- Avoid file paths, filenames, audio, tags, playlists, and listening history
- Be removable without reducing core functionality

## Accounts and network

Core functionality requires no account and no network connection. Local paths,
metadata, playback history, analysis, and library contents stay local unless the
user explicitly invokes an integration.
