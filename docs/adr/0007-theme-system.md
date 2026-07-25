# ADR 0007: Semantic Theme System

- Status: Accepted
- Date: 2026-07-25

## Context

PulseSeek requires polished light and dark themes, accessible contrast, and
theme-aware waveform and visualization colors.

## Decision

Use semantic CSS design tokens behind PulseSeek components. Tailwind CSS and
Radix UI provide implementation primitives but do not define the visual
identity.

Initial themes are PulseSeek Dark, PulseSeek Light, System, Midnight Blue, and
High Contrast.

## Consequences

- Feature components cannot hard-code palette colors.
- Themes switch without restart.
- Visualizations must consume semantic theme values.
- User-authored themes can be added later through a versioned format.
