# Technical Cache

The technical cache is the app's private SQLite database (`app-cache.sqlite`)
for application state that must not create manager items. It is one of the four
independent SQLite files described in ADR 0003.

## Ownership

- `pulseseek-cache` owns the shared SQLite infrastructure (migrations, backup,
  corruption recovery) and the `app-cache.sqlite` schema and worker.
- The Audio Player must start without the cache: startup treats an unavailable
  cache as a warning and continues.
- A technical-cache record is never a Sample Manager or Music Manager item.

## Layout

- Schema version 1 creates `cache_meta(key, value, updated_at_ms)` only.
  Feature record tables (waveform, recent folders, shortcuts, visualization
  settings) are added by the PRs that own them through later migrations.
- `migrations(version, applied_at_ms)` records the applied schema version.
  Migrations run in a transaction and are idempotent on repeat startup.
- The database is opened and migrated on `start`; every subsequent operation
  runs on a dedicated worker thread that owns the `rusqlite` connection.

## Failure modes

- **Corrupt database**: the file is quarantined as
  `<name>.corrupt-<timestamp>` and recreated; the cache reports
  `CacheStatus::Degraded` and remains usable.
- **Open failure** (permissions, missing directory): `start` returns an error,
  startup logs a warning, and the app continues without a cache.
- **Destructive migration**: an existing database is copied to
  `<name>.backup-<version>.sqlite` before any pending migration; a failed
  migration rolls back to the previous version.

## Boundaries

- No cross-database foreign keys. Later managers reference cache data by stable
  application identifiers only.
- The cache never touches the audio callback or the React thread; the worker
  connection keeps SQLite off the UI thread.
