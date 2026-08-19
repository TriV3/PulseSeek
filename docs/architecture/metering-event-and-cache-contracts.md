# Metering event and cache contracts

**Status:** normative technical design  
**Schema family:** metering-events-v1 and metering-cache-v1  
**Decision:** `docs/adr/0013-metering-architecture-and-contracts.md`  
**Traceability:** `docs/metering-traceability.md`

## Event families

Events use schema version 1, are narrow, and are independently subscribed. Wire names are `metering.session`, `metering.levels`, `metering.spectrum`, `metering.bands`, `metering.spectrogram`, `metering.waveform`, `metering.loudness`, `metering.true_peak`, `metering.stereo`, and `metering.diagnostics`. No all-meters event exists and raw complex FFT bins/audio samples never cross Tauri.

Each event has schema version, session/source identifiers, measurement point, sequence, source-sample timestamp, and validity. Sequence increases and timestamps do not decrease within session/family/subscription. Session changes reset ordering. Unknown schema versions are rejected without stopping playback.

| Family | Delivery |
|---|---:|
| session | on change |
| levels, spectrum, bands, stereo | 15–60 FPS |
| spectrogram, waveform | latest-only |
| loudness, true peak | continuous + display |
| diagnostics | 1–5 FPS |

Subscriptions have bounded independent mailboxes. Latest-only overflow replaces stale data and reports a drop. Continuous overflow never blocks producer, records a gap, and marks only that family invalid with queue-saturated reason. Unsubscribe is idempotent; dropping final receiver clears its mailbox.

Experimental events additionally expose formula, baseline, window, algorithm version, and validity.

## Cache index

SQLite stores metadata and coverage, not large frame arrays:

```sql
analysis_cache_index(
  cache_id TEXT PRIMARY KEY,
  source_fingerprint TEXT NOT NULL,
  algorithm_version TEXT NOT NULL,
  config_hash TEXT NOT NULL,
  product_kind TEXT NOT NULL,
  blob_path TEXT NOT NULL,
  sample_rate INTEGER NOT NULL,
  channels INTEGER NOT NULL,
  duration_samples INTEGER,
  coverage_state TEXT NOT NULL,
  checksum TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
)

analysis_cache_segment(
  cache_id TEXT NOT NULL,
  start_sample INTEGER NOT NULL,
  end_sample INTEGER NOT NULL,
  blob_offset INTEGER NOT NULL,
  blob_length INTEGER NOT NULL,
  validity TEXT NOT NULL,
  PRIMARY KEY(cache_id, start_sample)
)
```

Cache writes remain outside audio callback, use temporary files and atomic rename, and never create manager records.
