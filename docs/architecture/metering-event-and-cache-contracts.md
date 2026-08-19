# Metering event and cache contracts

**Status:** normative technical design  
**Schema family:** metering-events-v1 and metering-cache-v1  
**Decision:** `docs/adr/0013-metering-architecture-and-contracts.md`  
**Traceability:** `docs/metering-traceability.md`

## Event families

Events are versioned, narrow, and independently subscribed. There is no
all-meters event and raw complex FFT bins never cross the Tauri boundary.

| Family | Purpose | Default delivery |
|---|---|---:|
| session | source, format, continuity, validity | on change |
| levels | peak, RMS, balance, M/S | 15–60 FPS |
| spectrum | compact bins and display metadata | 15–60 FPS |
| bands | configured band values | 15–60 FPS |
| spectrogram | packed time rows | latest-only |
| colored waveform | covered temporal cells | latest-only |
| loudness | LUFS, LRA, duration, validity | continuous + display |
| true peak | sample peak and dBTP | continuous + display |
| stereo | correlation, width, goniometer data | 15–60 FPS |
| diagnostics | costs, queues, drops, degradation | 1–5 FPS |

Each payload has schema_version, algorithm_version, session_id, sequence,
timestamp_samples, sample_rate, channels, validity, and source_point where
applicable. A receiver can reject an unknown version without stopping playback.

## Payload shapes

```text
SessionEvent {
  schema_version
  session_id
  source_id
  point
  sample_rate
  channels
  state: Started | Paused | Resumed | LoopWrap | Reset | Stopped | Incomplete
  reason
}

SpectrumFrame {
  schema_version
  algorithm_version
  session_id
  sequence
  timestamp_samples
  fft_size
  window
  channel_view
  frequency_min
  frequency_max
  floor_db
  values_db: compact float array
  validity
}

BandEnergyFrame {
  schema_version
  session_id
  sequence
  timestamp_samples
  profile_id
  method
  channel_mode
  bands: [{ id, low_hz, high_hz, value, unit, validity }]
}

LoudnessFrame {
  schema_version
  algorithm_version
  session_id
  timestamp_samples
  momentary_lufs
  short_term_lufs
  integrated_lufs
  lra_lu
  duration_seconds
  gating_state
  validity
}

DiagnosticsFrame {
  schema_version
  session_id
  cpu_ms_p95
  cpu_ms_p99
  queue_depths
  visual_drops
  continuous_gaps
  active_products
  shared_products
  effective_quality
  reason
}
```

The exact Rust structs and Tauri serializers must preserve these semantics. A
renderer may interpolate presentation values, but it must not invent measurement
samples or hide invalidity.

## Subscription protocol

The UI requests a product family with a normalized ProductRequest. The Rust
application service returns a subscription_id and the current validity state.
Unsubscribe is idempotent. Dropping the last receiver clears the worker's
connection flag so an idle worker exits even if no new audio block arrives.

Every subscription has:

- requested source point and channel mode;
- product kind and algorithm version;
- FFT/window/hop where relevant;
- requested cadence and priority;
- mailbox capacity and latest-only/continuous policy.

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

The implementation may use the existing technical cache database, but must not
create manager records or cross-manager foreign keys. A migration records schema
version and can be rolled forward or invalidated safely.

## Blob format

Blobs are chunked by temporal coverage. The default chunk target is four
seconds, with a product-specific maximum byte size. A blob header contains:

- magic and format version;
- product kind and algorithm version;
- config hash and source fingerprint;
- sample rate, channels, start sample, duration;
- payload encoding and checksum.

Payloads use little-endian packed values with explicit quantization metadata.
Spectrum and band data may use float32; spectrogram display rows may use uint8
plus min/max dB metadata. A temporary file is fsynced when supported and
atomically renamed before SQLite coverage is committed. Recovery removes or
quarantines orphaned temporary blobs.

## Invalidation and retention

A path, size, modification time, stronger fingerprint, algorithm version, or
configuration change invalidates only incompatible coverage. Unknown coverage is
not silence. Cache writes are coalesced and never occur in the audio callback.

The user can clear one source, one product, one profile, or all technical cache.
Quota policy is visible. Oldest low-priority visual coverage is evicted before
continuous loudness/true-peak summaries. An offline analyzer may extend coverage
only when its algorithm/configuration matches or creates a new version.
