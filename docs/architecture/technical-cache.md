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
- Schema version 2 (PR-063) adds `waveform_cache(cache_key, source_size,
  source_modified_ms, algorithm_version, data, created_at_ms)` for versioned
  waveform data keyed by source path. Size, modified timestamp, and algorithm
  version validate every load; stale or corrupt rows are deleted on access.
- Schema version 3 (PR-074) adds
  `recent_folders(path TEXT PRIMARY KEY, name TEXT NOT NULL,
  last_opened_ms INTEGER NOT NULL)` for the bounded recent-folder history
  (FR-BR-011). Records are plain paths the user selected; the store is
  path-agnostic and never probes the filesystem, so a folder that disappears
  later stays listed and reopening it fails gracefully. The history is
  bounded to 10 entries and reorders by most-recently-opened timestamp, which
  is monotonic per database so ordering is deterministic. Display names are
  basenames and the service never logs paths or embeds a path in an error
  message.
- Schema version 4 (PR-080) adds
  `shortcut_mappings(action TEXT PRIMARY KEY, key TEXT,
  primary_modifier INTEGER, shift_modifier INTEGER, alt_modifier INTEGER)`.
  One transaction replaces the complete available shortcut profile. A
  case-insensitive unique chord constraint prevents conflicting mappings even if validation is
  bypassed. Reset stores the canonical defaults.
- Schema version 5 (PR-086) adds the singleton `visualization_settings` record. It stores only the
  built-in mode, enabled state, and quality policy; database checks reject unknown values.
- `migrations(version, applied_at_ms)` records the applied schema version.
  Migrations run in a transaction and are idempotent on repeat startup.
- The database is opened and migrated on `start`; every subsequent operation
  runs on a dedicated worker thread that owns the `rusqlite` connection.

## Failure modes

- **Corrupt database**: the file is quarantined as
  `<name>.corrupt-<timestamp>` and recreated; the cache reports
  `CacheStatus::Degraded` and remains usable.
- **Open failure** (permissions, missing directory): `start` returns an error,
  startup logs a warning, and the app continues without a cache. Recent-folder
  and shortcut commands then fall back to in-memory, session-only state so
  those features keep working without blocking startup. Shortcut defaults stay
  active when loading fails.
- **Destructive migration**: an existing database is copied to
  `<name>.backup-<version>.sqlite` before any pending migration; a failed
  migration rolls back to the previous version.

## Boundaries

- No cross-database foreign keys. Later managers reference cache data by stable
  application identifiers only.
- The cache never touches the audio callback or the React thread; the worker
  connection keeps SQLite off the UI thread.
