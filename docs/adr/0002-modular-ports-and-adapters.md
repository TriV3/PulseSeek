# ADR 0002: Modular Ports and Adapters

- Status: Accepted
- Date: 2026-07-25

## Context

PulseSeek must evolve from a folder player into several managers, DJ
integrations, visualizers, effect hosting, and a DAW bridge without coupling the
initial player to those later products.

## Decision

Use ports and adapters. Domain and application services depend on capability
interfaces. Tauri, React, SQLite, filesystem, audio, and integration libraries
are adapters.

The Audio Player does not depend on manager modules. Modules communicate using
typed commands, events, and stable identifiers.

## Consequences

- Core behavior is testable with fakes.
- Concrete frameworks can be replaced more safely.
- More interfaces and mapping code are required.
- Crates are introduced only for proven dependency boundaries.
