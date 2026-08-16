# ADR 0006: Separate Plugin Host and DAW Bridge

- Status: Accepted
- Date: 2026-07-25

## Context

PulseSeek must host effects and visualizers, and it must later expose the Sample
Manager inside a DAW. These are different trust boundaries and execution models.

## Decision

Treat them as independent systems:

1. A PulseSeek plugin host loads VST3 effects and versioned PulseSeek visualizer
   plugins.
2. A PulseSeek VST3 DAW bridge communicates with the desktop Sample Manager
   through versioned local IPC.

Audio Unit and CLAP are later targets. Plugin scanning should be isolated, and
safe mode disables third-party plugins.

## Consequences

- The desktop host and DAW bridge can evolve independently.
- Protocol versioning is required.
- Plugin failures are isolated from manager persistence where practical.
- More than one plugin-facing API must be maintained.
