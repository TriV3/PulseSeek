# PulseSeek Metering — Architecture and Contract Specification

**Status:** normative architecture specification  
**Version:** metering-architecture-v1  
**Companion:** spec/metering-functional-specification.md

## 1. Boundary and ownership

Rust owns:

- audio source adapters;
- capture taps and bounded queues;
- session and discontinuity state;
- DSP workers and product graph;
- subscription lifecycle;
- persistence and cache migrations;
- diagnostics and validity;
- Tauri commands/events and external capture permissions.

React owns:

- Browser/Meters workspace presentation;
- tile registry and layout;
- user controls and profile editing;
- Canvas renderer lifecycle;
- accessible labels, focus, keyboard behavior;
- display-only transforms.

No React component accesses filesystem, SQLite, audio devices, or raw playback
buffers. No tile owns a playback clock or DSP worker.

## 2. Source contract

The source port is:

~~~text
AudioAnalysisSource {
  start(request) -> SourceSession
  blocks(session) -> bounded block stream
  stop(session)
}

AnalysisBlock {
  schema_version
  session_id
  source_id
  source_kind
  measurement_point
  sample_rate
  channels
  first_sample
  frame_count
  interleaved_samples
  sequence
  discontinuity
}
~~~

Measurement points are Source, Monitor, ExternalApplication, SystemMix,
InputLoopback, and DAWBridge. External sources MUST declare identity and
capabilities. The adapter MUST reject unsupported channel layouts explicitly.

The callback side may only copy into a preallocated bounded ring and update
atomics. It MUST NOT allocate, lock, log, do I/O, run SQL, run DSP, communicate
with React, or manage subscribers.

## 3. Session state machine

States:

~~~text
Idle -> Running -> Paused -> Running
Running -> Resetting -> Running
Running -> Incomplete
Running -> Stopped -> Idle
~~~

Events:

~~~text
Start, AudioBlock, Pause, Resume, Seek, LoopWrap, FormatChange,
ChannelChange, Gap, Reset, Stop
~~~

Rules:

- Start creates a session identity.
- Pause stops the measurement clock but not the player.
- Resume continues the session.
- Seek creates a new session and resets continuity-dependent products.
- LoopWrap keeps the same session and records a seam marker.
- Format/channel/source changes create a new session.
- Gap marks continuous products incomplete.
- Stop releases subscriptions and capture resources.
- Reset can target all products, one tile, one product family, or one history.

## 4. Analysis engine topology

~~~mermaid
flowchart TB
  CB["Audio callback"] --> R["Bounded SPSC capture"]
  R --> E["AnalysisEngine"]
  E --> S["Session/discontinuity manager"]
  E --> G["Product dependency graph"]
  G --> F["FFT bank"]
  G --> D["Time-domain products"]
  G --> K["K-weighted loudness"]
  G --> T["True peak"]
  F --> V["Visual products"]
  D --> V
  K --> C["Continuous measurements"]
  T --> C
  V --> M["Latest-only mailboxes"]
  C --> N["Loss-intolerant mailbox"]
  M --> I["Tauri visual events"]
  N --> J["Tauri measurement events"]
  C --> W["Cache writer"]
  W --> Q["SQLite index and blobs"]
~~~

The engine uses dedicated workers or a bounded worker pool. A new worker is
allowed only when a measured dependency boundary requires it. There is never a
worker per tile.

## 5. Product graph and subscriptions

A ProductKey is:

~~~text
ProductKey {
  source_point
  channel_mode
  fft_size
  window
  hop
  product_kind
  algorithm_version
  configuration_hash
}
~~~

A subscriber requests a ProductKey plus cadence, priority, mailbox policy, and
validity requirements. The engine:

1. normalizes the request;
2. finds an identical product;
3. shares it or creates the smallest missing branch;
4. increments consumer count;
5. delivers a subscription_id and current validity;
6. stops the product after the last consumer is dropped.

Dropping a receiver clears a shared output-connected flag so an idle worker exits
without requiring another send attempt. Unsubscribe is idempotent.

## 6. Lanes and backpressure

### 6.1 Continuous lane

Used by loudness, LRA, true peak, and any product whose semantics require every
sample. It has bounded input and output queues, explicit gap detection, and
validity propagation. A lost block makes the result incomplete.

### 6.2 Visual lane

Used by Spectrum display, bands, waveform color, spectrogram, goniometer, and
experimental overlays. It is latest-only. A stale frame may be discarded without
affecting measurement state or playback.

### 6.3 Degradation order

1. stop unsubscribed products;
2. reduce visual cadence;
3. increase visual hop;
4. disable optional large FFT branches;
5. drop stale visual frames;
6. disable lowest-priority experimental overlays;
7. mark continuous measurements incomplete on real sample loss;
8. never block the callback.

Effective quality, reason, queue depths, drops, CPU p95/p99, memory, and
underruns are diagnostics products.

## 7. Tauri commands and events

Commands:

~~~text
metering.list_modules()
metering.create_tile(module_kind)
metering.remove_tile(tile_id)
metering.update_tile(tile_id, patch)
metering.set_profile(profile_id, patch)
metering.subscribe(tile_id, request)
metering.unsubscribe(subscription_id)
metering.reset(scope)
metering.list_sources()
metering.start_external_source(source_id)
metering.stop_external_source(source_id)
metering.export_profile(profile_id)
metering.import_profile(versioned_profile)
metering.clear_cache(scope)
~~~

Event families are separate:

- metering.session;
- metering.levels;
- metering.spectrum;
- metering.bands;
- metering.spectrogram;
- metering.waveform;
- metering.loudness;
- metering.true_peak;
- metering.stereo;
- metering.diagnostics.

Every event includes schema_version, algorithm_version where applicable,
session_id, sequence, timestamp_samples, validity, and source point.

Raw complex FFT bins and audio samples MUST NOT cross the Tauri boundary.

## 8. Layout and profile schemas

A layout is:

~~~text
MeterLayout {
  schema_version
  layout_id
  name
  workspace_height
  tiles: [
    {
      tile_id
      module_kind
      x, y, width, height
      profile_id
      priority
      visible
    }
  ]
}
~~~

A profile is:

~~~text
MeterProfile {
  schema_version
  profile_id
  name
  render_fps
  fft_defaults
  window_defaults
  smoothing_defaults
  band_profile_ids
  history_policy
  display_policy
  cache_policy
  experimental_modules
}
~~~

Migrations are field-by-field. Unknown fields are ignored and preserved where
possible. Invalid fields use documented defaults and expose a diagnostic.

## 9. Cache and SQLite contract

The technical cache is separate from all manager databases. It stores:

~~~sql
analysis_cache_index(
  cache_id PRIMARY KEY,
  source_fingerprint,
  algorithm_version,
  configuration_hash,
  product_kind,
  blob_path,
  sample_rate,
  channels,
  coverage_state,
  checksum,
  created_at,
  updated_at
)

analysis_cache_segment(
  cache_id,
  start_sample,
  end_sample,
  blob_offset,
  blob_length,
  validity,
  PRIMARY KEY(cache_id, start_sample)
)
~~~

Blob headers identify magic, format version, product kind, configuration hash,
source fingerprint, rate, channels, sample range, encoding, and checksum.
Writes use temporary files and atomic rename where supported. SQLite coverage is
committed only after a valid blob is available.

Only played/captured segments are written by live analysis. Unknown coverage is
not silence. Algorithm/configuration changes create new coverage or invalidate
only incompatible segments.

## 10. External source adapters

The first external adapter is macOS application-output capture. It MUST:

- request permission explicitly;
- show selected application;
- show capture-active state;
- provide immediate Stop;
- report application exit, permission loss, and format changes;
- release the capture on unsubscribe or shutdown;
- feed the same AnalysisBlock contract.

System mix, input/loopback, and DAW bus/track adapters are distinct future ports.
Application output capture MUST NOT be labeled as a precise internal DAW bus.

## 11. Error and validity contract

Errors are typed and localized to a source, product, tile, or subscription.
One tile error MUST NOT stop playback or other products.

Validity values:

~~~text
Measured | Estimated | Stale | Interpolated | Incomplete | Unavailable | Invalid
~~~

Validity propagates downstream. An invalid input cannot produce a valid output.
Unavailable source, permission denial, queue saturation, unsupported format,
configuration error, and continuous gap all have distinct diagnostic reasons.

## 12. Privacy and lifecycle

No audio or analysis data leaves the machine without an explicit integration.
External capture is never enabled implicitly. Cache clear operations are scoped
and visible. Shutdown releases subscriptions, workers, source adapters, file
handles, and cache writers in that order.

## 13. Required architecture tests

The implementation MUST test:

- bounded callback behavior;
- last-receiver idle-worker shutdown;
- compatible product sharing;
- incompatible branch creation;
- latest-only visual drops;
- continuous-gap invalidation;
- session transition matrix;
- event version rejection;
- layout/profile migration;
- cache crash recovery and checksum invalidation;
- source permission and Stop lifecycle.
