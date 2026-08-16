# PulseSeek Metering — DSP Specification

**Status:** normative algorithm specification  
**Version:** metering-dsp-v1  
**Companion:** spec/metering-functional-specification.md

## 1. Numerical conventions

- Audio samples are normalized floating point values in [-1, 1].
- Rates supported by the first release are 44.1, 48, 88.2, 96, and 192 kHz.
- Mono and stereo are supported; unsupported layouts follow the functional spec.
- All timestamps are integer sample positions in the source session.
- All dB values use 20 log10 for amplitude and 10 log10 for power.
- Values below the display floor are clamped for rendering only.
- NaN, infinity, invalid configuration, and invalid input propagate a validity
  flag and never become a numeric zero.
- Every product records algorithm version, source point, sample rate, channel
  mode, configuration hash, and validity.

## 2. Window functions and FFT

Supported windows:

- Rectangular: w[n] = 1.
- Hann: w[n] = 0.5 - 0.5 cos(2 pi n / (N - 1)).
- Hamming: w[n] = 0.54 - 0.46 cos(2 pi n / (N - 1)).
- Blackman-Harris 4-term with coefficients 0.42323, 0.49755, 0.07922,
  and 0.00168.
- Flat Top with the implementation's published 5-term coefficient set.

The implementation MUST publish the exact Flat Top coefficient set in code and
calibration results. A window is part of a ProductKey.

For N input samples x[n], windowed samples are y[n] = x[n] w[n]. The FFT is
real-to-complex with N/2 + 1 bins. The bin center is:

~~~text
f[k] = k * sample_rate / N
~~~

Coherent gain and power normalization:

~~~text
coherent_gain = sum(w[n]) / N
power_norm = sum(w[n] * w[n])
~~~

One-sided amplitude:

~~~text
amplitude[k] = abs(X[k]) / (N * coherent_gain)
amplitude[k] *= 2 for 0 < k < N/2
~~~

DC and Nyquist are not doubled. Power is based on the documented power_norm.
PSD divides power by sample rate and the window effective noise bandwidth.

The bank MUST support 2,048, 4,096, 8,192, and 16,384 points concurrently.
Plans and buffers are reused. A visual product MAY skip windows, but a
continuous product MUST consume every input block required by its specification.

## 3. Hop, overlap, and quality profiles

The target render cadence is independent from the continuous input cadence.

Defaults:

| Profile | Render FPS | Visual refresh | Overlap | Visual priority |
|---|---:|---:|---:|---|
| Eco | 15 | 15 FPS | 25% | low |
| Normal | 30 | 30 FPS | 50% | normal |
| High | 60 | 60 FPS | 75% | high |
| Expert | 1–120 | user | user | user |

For a window size N and requested overlap p, nominal hop is N(1-p). If the
sample rate and target cadence require a different hop, the scheduler records
the effective hop. The effective hop is visible in diagnostics.

Visual FPS MAY be reduced, and visual hops MAY be increased, under load. The
continuous lane MUST NOT silently change its integration semantics.

## 4. Channel transforms and spectrum modes

For stereo L/R:

~~~text
Mid  = (L + R) / sqrt(2)
Side = (L - R) / sqrt(2)
Mono = (L + R) / 2
~~~

Spectrum channel modes:

- L;
- R;
- energy sum: sqrt((L² + R²) / 2);
- mono;
- Mid;
- Side;
- L/R overlay;
- L/R difference and balance.

Energy sum is not a waveform mix and must be labeled accordingly. The
difference/balance view records its exact numerator and denominator and marks
near-zero denominators unavailable.

Raw complex bins stay in Rust. Phase is exported only to products that need it,
with explicit phase units and unwrap policy.

## 5. Temporal smoothing and history

For target x, previous value y, elapsed time dt, and time constant tau:

~~~text
alpha = 1 - exp(-dt / tau)
y_new = y + alpha * (x - y)
~~~

Attack and release use separate taus. Defaults are attack 5 ms and release
120 ms. Expert bounds are 0.5 ms to 5,000 ms.

Default histories:

- short average: 1 second;
- long average: 10 seconds;
- peak hold: 0 to 30 seconds decay or infinite;
- waveform cell history: governed by zoom and cache profile;
- spectrogram history: 30 seconds, Expert range 5 seconds to 10 minutes.

A reset clears all histories declared continuity-dependent by the lifecycle matrix.
A renderer decay after pause is not a measurement history update.

## 6. Band energy

Default band boundaries:

~~~text
Infrabass  20–40 Hz
Subbass    40–120 Hz
Bass       120–250 Hz
Low-mid    250–500 Hz
Mid        500–2,000 Hz
High-mid   2,000–4,000 Hz
Presence   4,000–8,000 Hz
Brilliance 8,000–16,000 Hz
Air        16,000–Nyquist
~~~

A profile may overlap bands or leave gaps. It must preserve the ordered
boundary list and a profile version.

Methods:

1. Bin power: sum corrected bin power for bin centers inside the band.
2. Integrated PSD: integrate PSD multiplied by each bin bandwidth.
3. Filtered RMS: apply a documented band-pass filter bank, square, average, and
   square-root over the configured time window.
4. Relative energy: divide the selected band energy by a selected total-energy
   denominator and mark the denominator in the output.

Channel modes are linked L/R, independent L/R, Mid/Side, and mono-compatible.
Every result includes lower/upper bounds, method, channel mode, unit,
normalization, and validity.

## 7. Spectrum display and tilt

Display tilt is:

~~~text
display_db[f] = measured_db[f] + slope_db_per_octave * log2(f / reference_hz)
~~~

Tilt is applied after product calculation. The reference frequency and slope are
stored in renderer settings. A tilted display cannot be used as an input to a
band, loudness, cache, or comparison product.

The display floor defaults to -160 dBFS and may be changed for rendering. The
cursor reports un-tilted and displayed values.

## 8. Colored waveform

The waveform is a multiresolution temporal pyramid. Each cell contains:

~~~text
start_sample, end_sample, peak, RMS, coverage, dominant_frequency,
dominant_band, confidence, color_mode, validity
~~~

Dominant frequency is the maximum smoothed band-energy value. Confidence is
peak divided by the sum of neighboring candidate bands, clamped to [0, 1].
The color modes are:

- log-frequency hue from 20 Hz to Nyquist;
- fixed active-band palette;
- RGB mixing of low, mid, and high energy;
- energy as brightness or alpha;
- phase-risk hatch/outline from a configurable correlation threshold.

The color transform never changes stored audio, peaks, RMS, or coverage. A cell
without played/captured coverage is unknown, not silent. A cell from an
incompatible algorithm/configuration is invalid and cannot be displayed as
current.

## 9. Spectrogram

A row is generated from a compatible spectrum product:

~~~text
session_id
sequence
timestamp_samples
fft_size
window
frequency_min
frequency_max
dynamic_range_db
encoding
values
palette_id
validity
~~~

The renderer owns a bounded row ring. Default history is 30 seconds. Values MAY
be packed uint8 with min/max metadata for display-only rows; analytical rows
use float32. Palette, gamma, range, and scroll direction are display transforms.
Timestamps determine the time axis; dropped rows are never replaced by invented
rows.

## 10. Loudness

The implementation is normative to ITU-R BS.1770-4 and EBU R128. It uses the
specified K-weighting pre-filter and RLB high-pass, channel weights, momentary
and short-term block durations, absolute gate, relative gate, integrated
accumulation, and LRA method. The exact coefficient and block implementation
must be covered by the validation spec before release.

Products:

- LUFS-M;
- LUFS-S;
- LUFS-I;
- LRA;
- duration and measured coverage;
- gating state;
- validity and incomplete state.

Pause freezes the integration clock. A/B loop wrap continues. Ordinary seek,
source change, format change, or continuous gap resets or invalidates according
to the functional lifecycle matrix.

## 11. True peak

Default true peak is 4x oversampled polyphase reconstruction. Expert MAY choose
8x only when the active performance budget permits it. The product reports:

- sample peak per channel and maximum;
- true peak dBTP per channel and maximum;
- oversampling factor;
- filter version;
- timestamp of maximum;
- validity and incomplete state.

Sample peak is never substituted for true peak. Inter-sample fixtures are
mandatory for conformance.

## 12. Stereo, correlation, and phase

Broadband correlation uses Pearson correlation over the configured window:

~~~text
rho = covariance(L, R) / sqrt(variance(L) * variance(R))
~~~

The result is clamped to [-1, 1]. Zero-variance input is unavailable, not zero.

Default width is:

~~~text
width_db = 20 * log10(RMS(Side) / RMS(Mid))
~~~

Near-zero Mid requires an explicit unavailable/overflow state. The goniometer
plots Mid on X and Side on Y. Frequency stereo uses the same formulas on
compatible per-channel FFT products. The default visual phase-risk threshold
is rho < -0.2 and is configurable. It is a display threshold, not a verdict.

Mono fold-down uses the normalized Mono transform and exposes both visual and
optional audition paths. L=-R must show cancellation immediately.

## 13. Experimental decision products

All experimental products use declared inputs, windows, baselines, formulas,
algorithm versions, and validity:

- occupancy field: instantaneous and rolling percentiles per band;
- transient/tonal contrast: short-window energy divided by local sustain energy;
- dynamic density: crest factor, RMS distribution, peak hold, occupancy;
- mono survivability: per-frequency correlation and M/S energy;
- coexistence matrix: overlap between selected snapshots or references;
- uncertainty field: measured, stale, interpolated, incomplete, unplayed;
- decision snapshot: synchronized values, settings, cursor, annotations;
- cost inspector: product graph, CPU, memory, queue, drops, and removable work.

These products MAY be enabled independently and MUST reuse compatible upstream
products. They MUST NOT emit automatic corrective instructions.

## 14. Numerical validity

Every frame includes a Validity value:

- measured;
- estimated;
- stale;
- interpolated;
- incomplete;
- unavailable;
- invalid.

Validity is propagated downstream. An invalid input cannot yield a valid
measurement. Rendering may show a fallback glyph, but it cannot replace invalid
data with zero.
