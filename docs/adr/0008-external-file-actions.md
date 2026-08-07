# ADR 0008: External File Actions

- Status: Accepted
- Date: 2026-08-07

## Context

PulseSeek must let the user reveal a file in the operating system file manager
(FR-FM-006) and open a file in another application (FR-FM-007). These actions
launch external processes, which is a security-sensitive capability. The
frontend must never receive a general process-launch API.

## Decision

Expose two narrow, single-file commands through the versioned command
envelope: `reveal_file` and `open_with`. Each accepts only a path string and
returns a typed result.

The behavior lives behind a domain port `ExternalActions` with a typed
`ExternalActionError`. The native adapter in `pulseseek-browser-fs` delegates
open-with to the `open` crate and reveals the file with a small
platform-specific command (`open -R` on macOS, `explorer /select` on Windows,
and `xdg-open` on the parent directory on other Unix-like systems). The
adapter validates the path exists before launching, so a missing file is
reported as `NotFound` without spawning a process.

The Tauri command layer wraps the adapter in an `ExternalService` that maps
domain errors to the standard boundary error contract. React only calls
`revealFile` and `openWith`; it has no way to launch an arbitrary process.

## Consequences

- React receives no general process-launch capability.
- Reveal and open-with are single-file actions on the primary selection.
- Platform-specific reveal behavior has a documented fallback (open the
  parent directory).
- A new structural dependency (`open` crate) is added to `pulseseek-browser-fs`.
- The adapter never embeds the raw path in user-facing messages.