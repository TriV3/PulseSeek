# ADR 0013: Real-time metering architecture and contracts

- Status: Accepted
- Date: 2026-08-19

## Context

PulseSeek metering combines playback-adjacent capture, shared DSP products,
continuous measurements, visual consumers, external monitoring, cache data, and
versioned Tauri contracts. These boundaries are expensive to change after
production code and user layouts depend on them. Metering must preserve the
Audio Player boundary, playback priority, privacy, and separation between
external capture systems.

## Decision

Metering is one Rust-owned analysis domain behind narrow typed application and
Tauri ports. React owns workspace presentation, tile configuration, accessible
controls, and display transforms. React never accesses audio buffers, devices,
files, databases, or capture permissions directly.

PulseSeek Source and Monitor are separate measurement points. ExternalApplication,
SystemMix, InputLoopback, and DAWBridge are separate source adapters. Application
output capture is never represented as precise DAW track or bus data. External
capture starts only after explicit user action, exposes identity and permission,
and stops immediately on request or source loss.

The audio callback only copies into preallocated bounded storage and updates
atomics. Capture overflow is observable. Visual queues are latest-only and may
drop stale frames. Continuous products consume every required block; a lost
block marks affected results incomplete. No callback or visual overload blocks
playback.

A shared ProductKey graph owns DSP products. Compatible subscribers share
products, and the final unsubscribe stops unused work. Continuous products use a
loss-intolerant lane. Visual products use a latest-only lane. Cache writes run
asynchronously and contain only played or captured coverage; cache data never
creates manager items or leaves the machine implicitly.

Events and persisted records carry explicit schema and algorithm versions,
session identity, source point, sequence, sample timestamp, and validity. Raw
audio and complex FFT bins do not cross the Tauri boundary. Unknown versions are
rejected without stopping playback.

## Consequences

- Playback remains independent of metering, manager databases, and external
  capture failures.
- Product sharing and lane policies are stable contracts for later DSP and UI
  PRs.
- Continuous gaps are visible rather than silently presented as complete data.
- New source adapters require explicit contracts, permissions, privacy review,
  and validation evidence.
- Schema, algorithm, cache, and requirement traceability must be versioned with
  each behavior change.

## Relationships

This ADR refines ADR 0002 for metering ports and adapters, ADR 0005 for callback
safety and playback priority, ADR 0006 for the separation of plugin hosting and
DAW bridge concerns, and ADR 0007 for accessible themed presentation.
