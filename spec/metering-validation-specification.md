# PulseSeek Metering — Validation Specification

**Status:** normative acceptance specification  
**Version:** metering-validation-v1  
**Companion:** spec/metering-functional-specification.md

## 1. Validation layers

A metering implementation is accepted only when all applicable layers pass:

1. mathematical unit and formula tests;
2. deterministic DSP fixture tests;
3. state/lifecycle tests;
4. product graph and subscription tests;
5. IPC/schema/cache tests;
6. UI behavior and accessibility tests;
7. performance and overload tests;
8. manual comparison against recognized meters.

A screenshot or smooth animation alone is never evidence of measurement correctness.

## 2. Fixture catalogue

The harness generates deterministic signals in memory and stores small WAV fixtures
only where decoder and persistence paths must be exercised.

Required fixtures:

| ID | Signal | Purpose |
|---|---|---|
| F-001 | silence | floor, unavailable, no false energy |
| F-002 | 1 kHz at -18 dBFS | amplitude calibration |
| F-003 | 30 Hz sine | infrabass boundary |
| F-004 | 50 Hz + 10 kHz | separated band energy |
| F-005 | pink noise | PSD and spectral slope |
| F-006 | L=R | correlation +1, Mid |
| F-007 | L=-R | correlation -1, mono cancellation |
| F-008 | left-only | balance and channel isolation |
| F-009 | right-only | balance and channel isolation |
| F-010 | impulse | transient, peak, true peak |
| F-011 | inter-sample peak | true-peak conformance |
| F-012 | calibrated BS.1770/EBU sequence | LUFS and LRA |
| F-013 | frequency sweep | mapping, spectrogram, color |
| F-014 | transient plus sustain | transient/tonal contrast |
| F-015 | alternating phase bands | frequency phase survivability |
| F-016 | long mixed program | integrated state, cache coverage |

Every fixture records generator version, sample rate, channels, duration, level,
expected result, tolerance, and license/provenance.

Each required fixture runs at 44.1, 48, 88.2, 96, and 192 kHz. Stereo fixtures
also run mono where the transform is meaningful. Performance or resource-heavy
fixtures may use a reduced duration at 192 kHz, but must retain expected-value
coverage.

## 3. DSP tolerances

Initial acceptance tolerances:

- bin frequency: within one FFT bin;
- bin-centered calibrated sine amplitude: ±0.1 dB;
- window normalization: ±0.1 dB against reference;
- correlation +1/-1: ±0.001;
- balance isolation: channel leakage below -120 dB in the ideal fixture;
- true peak: ±0.1 dBTP against reference vectors;
- LUFS and LRA: ±0.1 LU unless the official reference sequence publishes a
  different tolerance;
- band boundary classification: no more than one boundary bin ambiguity,
  explicitly reported;
- smoothing: time constant reaches 63.2% of the step response within ±5%;
- waveform coverage: no unplayed cell may be marked measured;
- spectrogram timestamp: monotonic and no row may claim an absent timestamp.

Any altered tolerance requires a versioned decision and new fixture evidence.

## 4. Module acceptance

### 4.1 Workspace and tiles

Canonical default tiles are Spectrum, Band Energy, Colored Waveform,
Spectrogram, Loudness, True Peak, Stereo, and Diagnostics.

- Browser/Meters switch preserves Browser state and playback.
- Eight default tiles can be added and removed.
- Duplicate tiles receive distinct IDs and share compatible products.
- Resize and maximize preserve settings and subscriptions.
- Keyboard focus, labels, high contrast, reduced motion, and non-color validity
  encodings are verified manually and with React behavior tests.

### 4.2 Spectrum

- All four FFT sizes and all required windows pass frequency/amplitude fixtures.
- All channel modes pass L=R, L=-R, and single-channel fixtures.
- Instant, smoothing, average, and peak hold histories are distinguishable.
- Tilt changes pixels only, never shared values.
- Closing the last Spectrum consumer stops its unused product.

### 4.3 Bands

- Default boundaries and custom profiles round-trip.
- Gaps, overlaps, methods, channel modes, and units are visible and deterministic.
- Overlay and standalone tile agree for identical requests.
- Configuration changes create a new configuration hash.

### 4.4 Colored waveform

- Coverage grows only as playback/capture reaches a segment.
- Zoom changes resolution without changing the segment meaning.
- All color modes reproduce from cached data and remain readable monochrome.
- Stale, incomplete, invalid, and unplayed cells are visually distinct.

### 4.5 Spectrogram

- Rows reuse compatible Spectrum data.
- History is bounded, timestamps are monotonic, and drops do not invent rows.
- Cursor reports frequency, time, value, and validity.

### 4.6 Loudness and true peak

- LUFS-M/S/I, LRA, gating, duration, and dBTP match fixtures.
- Pause freezes integrated measurement.
- Ordinary seek resets as specified.
- A/B loop wrap does not reset.
- Continuous gaps mark affected values incomplete.
- Sample peak and true peak are shown as separate products.

### 4.7 Stereo and decision products

- L=R produces +1 correlation; L=-R produces -1 and mono cancellation.
- Goniometer and frequency stereo use documented normalization.
- Occupancy, survivability, transient/tonal, density, snapshots, uncertainty,
  coexistence, and cost views expose their inputs and versions.
- Experimental views can be disabled without stopping core meters.

## 5. Lifecycle and failure tests

Required scenarios:

- pause, resume, ordinary seek, backward seek, A/B wrap, file change, rate
  change, channel change, source loss, source stop;
- visual mailbox saturation;
- continuous-lane gap;
- last receiver dropped while worker is idle;
- source permission denied and revoked;
- cache write interrupted before rename;
- corrupted checksum and unknown schema version;
- invalid configuration and profile migration;
- one tile error while other tiles and playback remain healthy.

## 6. Performance budgets

Every benchmark records hardware, OS, build, sample rate, channels, profiles,
tile layout, FFT sizes, CPU p50/p95/p99, memory, queue depth, visual drops,
continuous gaps, and audio underruns.

Required scenarios:

| Scenario | Acceptance |
|---|---|
| Apple Silicon M1, 8 GB, stereo 48 kHz, Normal, 8 tiles | no audio underrun, bounded queues |
| Apple Silicon M1, 8 GB, stereo 48 kHz, High, 6 tiles | 60 FPS target or visible degradation |
| stereo 96 kHz, all standard FFT products | no continuous gap |
| stereo 192 kHz | visual degradation may activate, never silently |
| 10-minute spectrogram history | bounded memory and stable cursor |
| partial live cache close/reopen | exact coverage restoration |
| external source start/stop | errors isolated from playback |

Performance changes are accepted only with before/after measurements. A
renderer that looks smooth while continuous measurements are incomplete fails.

## 7. Manual external comparison

Compare at minimum Spectrum, LUFS-M/S/I, LRA, sample peak, true peak,
correlation, goniometer, mono fold-down, and band energy with a recognized meter.
Record tool/version, settings, sample rate, fixture, observed value, expected
value, tolerance, and explanation for every discrepancy.

## 8. Traceability requirement

The release checklist MUST enumerate every requirement ID from the functional
specification and identify:

- owning PR;
- implementation file/module;
- automated test;
- manual check where applicable;
- algorithm/cache/event document;
- known limitation.

A requirement without evidence is not accepted as complete.
