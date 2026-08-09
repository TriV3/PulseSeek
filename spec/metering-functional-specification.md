# PulseSeek Metering — Functional Specification

**Status:** normative and exhaustive product specification  
**Version:** 1.0-draft  
**Scope:** live audio metering, visual analysis, configurable Meters workspace

This document is the authority for user-visible behavior. The DSP equations,
threading, event schemas, persistence format, and validation tolerances are
specified in the companion documents:

- spec/metering-dsp-specification.md
- spec/metering-architecture-specification.md
- spec/metering-validation-specification.md

This document refines section 4.5 and the `FR-VS-*` requirements in
`spec/functional-specification.md`. It does not replace the product-level
specification: Audio Player independence, manager separation, privacy, file
safety, accessibility, and global performance constraints remain applicable.
The implementation plan is the authoritative mapping from these requirements
to small, ordered PRs.

## 1. Normative language and principles

The words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY have their usual normative
meaning. A requirement is not complete until its acceptance evidence is linked
in the implementation plan.

Product principles:

1. The application displays measurements and relationships; it does not decide
   whether a mix is artistically correct.
2. Every visualization is configurable, but customization cannot bypass
   callback safety, bounded memory, or measurement validity.
3. Shared products are calculated once and consumed by many tiles.
4. Unknown, stale, interpolated, incomplete, and measured data are distinct.
5. Playback has priority over visualization and background analysis.
6. Every visible value has a defined source point, unit, time scope, and validity.
7. Offline analysis and external monitoring reuse this design without duplicating
   the live DSP engine.

## 2. Scope and non-goals

### 2.1 First live scope

The first live release includes:

- Meters workspace replacing the lower Browser/File List area;
- Spectrum Analyzer;
- configurable Band Energy;
- colored waveform;
- Spectrogram;
- Loudness and True Peak;
- Stereo, Mid/Side, correlation, goniometer, and mono survivability;
- shared FFT and product subscriptions;
- Eco/Normal/High/Expert profiles;
- diagnostics and adaptive quality;
- progressive coverage cache;
- macOS external application monitoring behind a permissioned adapter.

### 2.2 Explicit non-goals

The first live release MUST NOT:

- emit an automatic verdict such as 'fix this frequency' or 'compress this';
- perform automatic EQ, compression, limiting, or phase correction;
- infer individual stems from a stereo master;
- promise precise DAW track/bus/pre-fader data from application-output capture;
- require a manager database to play or meter a file;
- silently capture external applications;
- declare a full-file integrated result when coverage is incomplete.

## 3. Workspace and tile system

### 3.1 Lower workspace

The lower portion of the player has one active workspace:

- Browser: existing folder tree and File List;
- Meters: configurable tile workspace.

The upper waveform, seek, transport, playback mode, volume, and output controls
remain available in both workspaces.

FR-MW-001: The user MUST be able to switch Browser/Meters with mouse and keyboard.
FR-MW-002: Switching MUST preserve Browser selection, folder expansion, scroll,
and File List state.
FR-MW-003: Switching MUST preserve tile layout, tile settings, subscriptions,
and current session validity.
FR-MW-004: A workspace switch MUST NOT stop playback or reset the playhead.
FR-MW-005: Meters MUST provide add, remove, duplicate, reorder, resize, and
maximize operations.
FR-MW-006: A tile MUST have a stable tile identifier independent of position.
FR-MW-007: A layout MUST be saveable, renameable, restorable, resettable, and
deletable.
FR-MW-008: Removing a tile MUST unsubscribe only from products no longer used.
FR-MW-009: The catalogue MUST distinguish core modules from experimental modules.
FR-MW-010: The workspace MUST show an empty state, loading state, unavailable
state, incomplete state, and degraded state.
FR-MW-011: Every tile control MUST be keyboard accessible and focus-visible.
FR-MW-012: A tile MUST expose title, module, source point, units, settings access,
validity, and active quality in its header.
FR-MW-013: A maximized tile MUST be restorable to its previous grid position.
FR-MW-014: Layout dimensions MUST have minimum and maximum bounds and remain
usable when the window is resized.

### 3.2 Tile configuration

A tile instance consists of:

~~~text
tile_id
module_kind
profile_id
source_point
channel_mode
product_request
renderer_settings
priority
visibility
schema_version
~~~

FR-MW-015: Users MUST be able to edit settings at tile scope without modifying
other tiles.
FR-MW-016: Users MUST be able to apply a profile to one tile, a selected group,
or the whole workspace.
FR-MW-017: Expert settings MUST be exportable and importable as versioned data.
FR-MW-018: Invalid imported fields MUST fall back individually and report the
fallback without discarding valid fields.
FR-MW-019: Experimental modules MUST be opt-in and visually identified.
FR-MW-020: Removing a module MUST not delete its saved settings unless the user
explicitly deletes the module configuration.

## 4. Audio sources, measurement points, and lifecycle

### 4.1 Sources

The first source is PulseSeek playback. Future sources use the same adapter port:

- PulseSeek Source;
- PulseSeek Monitor;
- selected macOS application output;
- system mix;
- input/loopback;
- DAW bridge bus or track.

FR-MS-001: Source mode MUST measure decoded audio before resampling, gain, and output.
FR-MS-002: Monitor mode MUST measure after resampling and before user volume.
FR-MS-003: External source mode MUST show application/device identity and permission.
FR-MS-004: Mono and stereo MUST be supported in the first release.
FR-MS-005: Unsupported channel layouts MUST be rejected or reduced by an explicit
policy shown to the user.
FR-MS-006: Every block MUST carry source, session, point, sample rate, channels,
first sample, frame count, sequence, and discontinuity metadata.
FR-MS-007: A normal seek MUST start a new measurement session.
FR-MS-008: A/B loop wrap MUST remain in the same measurement session.
FR-MS-009: File, source, rate, or channel changes MUST start a new session.
FR-MS-010: Pause MUST freeze measurement clocks while instant visual presentation
decays progressively toward silence.
FR-MS-011: Pause MUST NOT inject synthetic silence into LUFS, LRA, averages, or
coverage.
FR-MS-012: A continuous-lane gap MUST mark affected measurements incomplete.
FR-MS-013: The user MUST be able to reset all measurements or one tile explicitly.
FR-MS-014: A reset MUST display its scope and timestamp to the user.

### 4.2 State matrix

| Event | Instant visuals | Smoothing/hold | LUFS/LRA | Cache coverage |
|---|---|---|---|---|
| pause | decay presentation | freeze/decay by setting | freeze clock | no new segment |
| resume | continue | continue | continue | append |
| normal seek | reset | reset | reset | new segment/session |
| A/B wrap | continue | continue | continue | same session |
| file/source change | reset | reset | reset | new source key |
| visual drop | latest frame | unchanged | unchanged | unchanged |
| continuous gap | may continue visually | valid only if marked | incomplete | invalid segment |

## 5. Spectrum Analyzer

Inputs: shared per-channel FFT product and display settings.

FR-SP-001: FFT sizes MUST include 2,048, 4,096, 8,192, and 16,384.
FR-SP-002: Windows MUST include Hann, Hamming, Blackman-Harris, Flat Top, and
advanced Rectangular.
FR-SP-003: The user MUST choose logarithmic or linear frequency axis.
FR-SP-004: The user MUST choose L, R, energy sum, mono, Mid, Side, L/R overlay,
or L/R difference/balance.
FR-SP-005: Instant spectrum, smoothing, short average, long average, and peak
hold MUST be independently selectable.
FR-SP-006: Attack, release, average duration, hold duration, and decay MUST be
independently configurable within validated bounds.
FR-SP-007: Magnitude dBFS, power dBFS, and PSD dBFS/Hz MUST be selectable when
the selected product supports that unit.
FR-SP-008: Display tilt MUST be configurable and MUST NOT modify shared products.
FR-SP-009: Frequency and amplitude ranges MUST be editable and resettable.
FR-SP-010: The display MUST show FFT size, window, channel mode, unit, floor, tilt,
quality, and validity.
FR-SP-011: A cursor MUST report frequency, value, channel mode, and timestamp.
FR-SP-012: The user MUST be able to freeze a frame for a decision snapshot without
stopping live analysis.
FR-SP-013: Two compatible spectrum tiles MUST share their FFT product.
FR-SP-014: Raw complex bins MUST remain in Rust and MUST NOT be sent to React.

Acceptance: calibrated tones appear at expected frequency and level, all modes
produce documented values, visual drops do not affect playback, and closing the
last consumer stops the unused product.

## 6. Band Energy

Default bands:

| Name | Lower bound | Upper bound |
|---|---:|---:|
| Infrabass | 20 Hz | 40 Hz |
| Subbass | 40 Hz | 120 Hz |
| Bass | 120 Hz | 250 Hz |
| Low-mid | 250 Hz | 500 Hz |
| Mid | 500 Hz | 2 kHz |
| High-mid | 2 kHz | 4 kHz |
| Presence | 4 kHz | 8 kHz |
| Brilliance | 8 kHz | 16 kHz |
| Air | 16 kHz | Nyquist |

FR-BE-001: Users MUST be able to rename, add, delete, reorder, and edit bands.
FR-BE-002: Gaps and overlaps MUST be allowed and visibly represented.
FR-BE-003: Methods MUST include corrected bin power, integrated PSD, filtered
RMS, and relative energy.
FR-BE-004: The UI MUST display the active method, channel mode, bounds, and unit.
FR-BE-005: Channel modes MUST include linked L/R, independent L/R, Mid/Side, and
mono-compatible.
FR-BE-006: Bands MUST be available as a standalone tile and Spectrum overlay.
FR-BE-007: The musical-band preset MUST use this same versioned band model.
FR-BE-008: A band profile MUST be exportable/importable and migratable.
FR-BE-009: Editing a band profile MUST not change previously stored cache data
without creating a new configuration hash.
FR-BE-010: The user MUST be able to display absolute, relative, rolling average,
and peak-held band energy.

Acceptance: boundary tones, overlapping profiles, gap profiles, and all channel
modes produce deterministic expected values.

## 7. Colored waveform

FR-CW-001: Only played or captured temporal cells MAY receive analysis colors.
FR-CW-002: Unplayed, stale, interpolated, incomplete, and measured cells MUST be
visually distinct.
FR-CW-003: Color modes MUST include dominant frequency, fixed band palette, RGB
low/mid/high energy, and energy brightness/alpha.
FR-CW-004: The waveform shape MUST remain readable in monochrome.
FR-CW-005: Zoom MUST select an appropriate temporal resolution without changing the
meaning of already measured cells.
FR-CW-006: Color settings MUST be independent of audio data and exportable.
FR-CW-007: A cell MUST expose coverage, peak, RMS, dominant band, confidence, and
validity to a cursor/inspection view.
FR-CW-008: Color mapping MUST expose palette, range, gamma, saturation, and
color-blind-safe mode.
FR-CW-009: A cache reload MUST reproduce colors from the stored algorithm/config
version or mark the cells incompatible.

## 8. Spectrogram

FR-SG-001: Rows MUST reuse compatible Spectrum products.
FR-SG-002: The user MUST configure FFT size, frequency range, dynamic range,
palette, scroll direction, speed, and history duration.
FR-SG-003: History MUST be bounded in memory and rows MUST carry timestamps.
FR-SG-004: A row drop MUST never create an incorrect time axis.
FR-SG-005: Palette and dynamic range MUST be display transforms.
FR-SG-006: The cursor MUST report time, frequency, value, and validity.
FR-SG-007: The user MUST be able to freeze a synchronized snapshot.
FR-SG-008: Expert history duration MUST be limited by a visible memory budget.

## 9. Loudness and true peak

FR-LD-001: Loudness MUST follow BS.1770-4 and EBU R128 semantics.
FR-LD-002: The implementation MUST expose LUFS-M, LUFS-S, LUFS-I, LRA, duration,
gating state, and validity.
FR-LD-003: Sample peak MUST be shown per channel and maximum.
FR-LD-004: True peak MUST be shown in dBTP per channel and maximum.
FR-LD-005: The true-peak oversampling factor and filter version MUST be visible.
FR-LD-006: Pause and visual drops MUST NOT affect integrated loudness.
FR-LD-007: A continuous sample gap MUST invalidate affected integrated values.
FR-LD-008: The tile MUST show whether a result is session-live, partial, or complete.
FR-LD-009: The user MUST be able to reset loudness independently from visual tiles.
FR-LD-010: Compact and historical live views MUST be available.
FR-LD-011: Calibration tolerances MUST be published before conformance is claimed.

## 10. Stereo, Mid/Side, correlation, and phase

FR-ST-001: The tile MUST show L/R balance, Mid energy, Side energy, width, and
broadband correlation.
FR-ST-002: The tile MUST show a goniometer/vectorscope mode.
FR-ST-003: Correlation MUST be available over a configurable time window.
FR-ST-004: Frequency correlation, width, and M/S MUST reuse per-channel FFT data.
FR-ST-005: Mono-compatible fold-down MUST be displayable and optionally auditionable.
FR-ST-006: L=-R MUST produce an unmistakable cancellation visualization.
FR-ST-007: Phase-risk thresholds MUST be configurable and labeled as visual
thresholds, never as an automatic quality verdict.
FR-ST-008: Zero-variance correlation MUST be shown unavailable rather than zero.
FR-ST-009: The tile MUST show the selected normalization and time/frequency scope.

## 11. Decision-support visualizations

These are P1 experimental modules and MUST be opt-in:

FR-DS-001: Spectral occupancy field with instantaneous value, percentile, and
user-selected baseline.
FR-DS-002: Mono survivability field with per-frequency correlation and M/S energy.
FR-DS-003: Transient/tonal contrast ratio over time and selected bands.
FR-DS-004: Dynamic density map combining crest factor, RMS distribution, peak
hold, and spectral occupancy.
FR-DS-005: Decision snapshots storing synchronized tile state, cursor, settings,
and user annotations.
FR-DS-006: Uncertainty/coverage layer distinguishing measured, stale, interpolated,
incomplete, and unplayed data.
FR-DS-007: Cost inspector showing shared products, CPU, memory, queues, drops,
and products that would stop when a tile is removed.
FR-DS-008: Coexistence/masking matrix comparing selected snapshots or future
reference sources without claiming source separation.
FR-DS-009: All experimental views MUST expose formula, baseline, window, and
algorithm version.
FR-DS-010: Experimental views MUST be removable without stopping core meters.

## 12. Profiles, quality, and customization

FR-CF-001: Eco, Normal, and High MUST target 15, 30, and 60 render FPS.
FR-CF-002: Expert MUST permit validated custom render FPS from 1 to 120.
FR-CF-003: FFT size, window, hop, overlap, smoothing, history, display floor,
palette, channel view, source point, and priority MUST be configurable where
supported.
FR-CF-004: Quality degradation MUST follow the ordered policy in the architecture
specification and MUST be visible.
FR-CF-005: Profiles MUST be exportable/importable with schema and algorithm versions.
FR-CF-006: User settings MUST be migrated field by field.
FR-CF-007: A profile MUST declare its CPU/memory budget and history policy.
FR-CF-008: A user override MUST never disable callback safety or validity marking.

## 13. External monitoring

FR-EX-001: External monitoring MUST be explicitly started by the user.
FR-EX-002: macOS application capture MUST show selected application and permission.
FR-EX-003: Stop MUST release the capture immediately.
FR-EX-004: Source disappearance, permission loss, and format changes MUST be
visible and recoverable.
FR-EX-005: System mix, loopback, input, and DAW bridge MUST remain separate adapters.
FR-EX-006: Application output capture MUST NOT be represented as a precise DAW
track or bus measurement.

## 14. Persistence and privacy

FR-CA-001: Only played/captured coverage MAY be stored by live analysis.
FR-CA-002: Large analysis data MUST use versioned blobs referenced by SQLite.
FR-CA-003: Cache identity MUST include source fingerprint, algorithm version,
configuration hash, product kind, rate, channels, and checksum.
FR-CA-004: Cache writes MUST be asynchronous and crash recoverable.
FR-CA-005: Unknown coverage MUST NOT be interpreted as silence.
FR-CA-006: The user MUST be able to clear cache by source, product, profile,
or globally.
FR-CA-007: No audio, path, or analysis data MAY leave the machine without an
explicit user-invoked integration.

## 15. Accessibility and errors

FR-UI-001: Every primary control MUST be keyboard reachable.
FR-UI-002: Color MUST NOT be the only encoding of validity, phase risk, coverage,
or degraded state.
FR-UI-003: Reduced-motion and high-contrast settings MUST be honored.
FR-UI-004: Error, permission, incomplete, unavailable, and degraded states MUST
have text and accessible labels.
FR-UI-005: The UI MUST never display an invalid measurement as a valid number.
FR-UI-006: The user MUST be able to inspect the reason for a degradation.

## 16. Acceptance gate

The metering release is accepted only when:

1. Every P0/P1 requirement has a PR owner, test owner, and specification link.
2. Each module has deterministic fixtures and published tolerances.
3. Compatible tiles demonstrate product sharing in diagnostics.
4. Removing a last subscriber stops the product and worker.
5. Playback remains stable under all quality profiles and overload states.
6. Pause, seek, A/B loop, source changes, and gaps follow the state matrix.
7. Cache reload preserves valid coverage and rejects incompatible data safely.
8. External capture is permissioned, visible, stoppable, and isolated.
9. Experimental views are configurable, opt-in, and never issue automatic verdicts.
10. The implementation plan, technical specs, tests, and user controls are
    updated in the same PR whenever their behavior changes.
