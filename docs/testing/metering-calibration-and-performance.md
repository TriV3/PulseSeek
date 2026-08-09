# Metering calibration and performance validation

**Status:** normative test plan  
**Reference implementation:** metering-dsp-v1

## Deterministic fixtures

The harness generates signals in memory and stores only small versioned WAVs when
a decoder-path test is required:

- silence;
- 1 kHz sine at -18 dBFS;
- 30 Hz sine;
- 50 Hz plus 10 kHz;
- pink noise;
- L=R;
- L=-R;
- left-only and right-only;
- impulse;
- inter-sample true-peak signal;
- calibrated EBU/ITU sequences where redistribution is permitted.

Every fixture records sample rate, channels, duration, level, generator version,
and expected tolerance.

## DSP assertions

- Frequency mapping agrees with k * sample_rate / N within one bin.
- Hann coherent-gain normalization reproduces the calibrated sine level within
  0.1 dB at a bin-centered frequency.
- DC and Nyquist are not doubled; interior one-sided bins are doubled once.
- Power and PSD remain consistent when sample rate changes.
- Band boundaries, gaps, and overlaps are deterministic.
- Mid/Side and mono transforms preserve the documented normalization.
- Correlation is +1 for L=R, -1 for L=-R, and unavailable for silence.
- A mono fold-down exposes cancellation for L=-R.
- True peak agrees with the reference vectors within 0.1 dBTP.
- LUFS-M/S/I and LRA use published tolerances before implementation is marked
  conformant; the default acceptance target is 0.1 LU unless the reference
  suite requires a different documented tolerance.
- Pause freezes integrated loudness; ordinary seek resets it; A/B wrap preserves it.
- Unknown, stale, interpolated, incomplete, and measured coverage remain distinct.

## Contract and lifecycle tests

- A bounded visual mailbox drops stale frames without blocking the producer.
- A continuous gap marks the affected measurement incomplete.
- Unsubscribing the last receiver stops an idle worker.
- Two compatible tiles share one product; incompatible requests create only the
  necessary additional branch.
- A tile removal does not stop a product still used by another tile.
- Session changes reset only products declared continuity-dependent.
- Cache crash recovery cannot expose a partial blob as valid coverage.
- Unknown event versions are rejected without stopping audio.
- The Browser/Meters switch preserves Browser state and tile subscriptions.

## Performance matrix

The benchmark report includes hardware, OS, build profile, sample rate,
channels, tile layout, quality profile, FFT sizes, CPU p50/p95/p99, memory,
queue depth, visual drops, continuous gaps, and audio underruns.

Minimum scenarios:

| Scenario | Required observation |
|---|---|
| M1/8 GB, stereo, 48 kHz, Normal, 8 tiles | no audio underrun; bounded queues |
| M1/8 GB, stereo, 48 kHz, High, 6 tiles | 60 FPS target or visible degradation |
| stereo, 96 kHz, largest FFTs | no continuous gap |
| stereo, 192 kHz | visual degradation may activate and is visible |
| 10-minute history and cache reopen | bounded memory and correct coverage |
| external source start/stop | permission/source errors do not affect playback |

No optimization is accepted only because an animation looks smooth. The report
must show the audio callback remains within budget and that measurement validity
is preserved.

## Manual validation

Compare Spectrum, LUFS, LRA, dBTP, correlation, phase, and mono behavior against
a recognized external meter with versions, settings, sample rate, fixture,
observed result, tolerance, and explanation of every discrepancy recorded.

## Documentation gate

A DSP or transport PR is not complete until it updates:

- the relevant algorithm formula and units;
- event and cache schemas if changed;
- lifecycle/reset behavior;
- fixtures and expected tolerances;
- performance results;
- visible controls and accessibility behavior;
- known limitations and follow-up work.
