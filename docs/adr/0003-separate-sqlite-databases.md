# ADR 0003: Separate SQLite Databases

- Status: Accepted
- Date: 2026-07-25

## Context

Samples, music, and playlists have different lifecycles and metadata. The Audio
Player must remain available when a manager is absent or damaged.

## Decision

Use four independent SQLite files:

- `app-cache.sqlite`
- `samples.sqlite`
- `music.sqlite`
- `playlists.sqlite`

Use `rusqlite` and a dedicated worker for each database. Each database owns its
migrations and recovery. Cross-manager references use stable application IDs,
not cross-database foreign keys.

## Consequences

- Managers can fail, migrate, back up, and recover independently.
- Cross-manager joins must occur in application services.
- Referential integrity across managers is implemented explicitly.
- The Audio Player has no manager-database startup dependency.
