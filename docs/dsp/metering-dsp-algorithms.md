# Metering DSP algorithms

**Status:** normative technical design  
**Algorithm version:** metering-dsp-v1  
**Decision:** `docs/adr/0013-metering-architecture-and-contracts.md`  
**Traceability:** `docs/metering-traceability.md`

## FFT and amplitude normalization

Supported sizes are 2,048, 4,096, 8,192, and 16,384. For input x[n], window
w[n], and FFT X[k]:

```text
coherent_gain = sum(w[n]) / N
power_norm    = sum(w[n]^2)
f_k           = k * sample_rate / N
```

For a real, one-sided spectrum, amplitude is:

```text
A[k] = |X[k]| / (N * coherent_gain)
A[k] *= 2 for interior bins only
```

DC and Nyquist bins are not doubled. Power uses power_norm; PSD divides by
sample rate and effective noise bandwidth. All dB conversions clamp to a
configurable floor, default -160 dBFS. NaN and infinite values invalidate the
frame instead of entering a renderer.

Default windows are Hann, Hamming, Blackman-Harris, Flat Top, and advanced
Rectangular. Blackman-Harris uses coefficients `0.42323`, `0.49755`, `0.07922`,
and `0.00168`. Flat Top uses the published five-term coefficients
`0.21557895`, `0.41663158`, `0.27726316`, `0.083578944`, and `0.006947368`.
Window choice is part of the product key. Tilt is applied only at presentation
time.

Calibration covers every size/window combination with bin-centered tones. The
measured coherent gains converge to 1.0 Rectangular, 0.5 Hann, 0.54 Hamming,
0.42323 Blackman-Harris, and 0.21557895 Flat Top. Corresponding mean-square
window powers converge to 1.0, 0.375, 0.3974, 0.30604, and 0.17522. Amplitude
acceptance is ±0.1 dB. Invalid sample rates, frame lengths, or non-finite input
produce typed FFT errors instead of numeric output.

## Channel transforms

The initial channel topology is mono or stereo:

```text
Mid  = (L + R) / sqrt(2)
Side = (L - R) / sqrt(2)
Mono = (L + R) / 2
```

The Spectrum supports L, R, energy sum, mono, Mid, Side, L/R overlay, and L/R
difference/balance. Energy sum is calculated per spectral bin as
`sqrt((L² + R²) / 2)` and is never treated as a waveform mix. Overlay and
balance products reuse L/R results rather than creating another transform.

Each shared FFT bank is bound to one immutable source-stream identity and keys
its branches by FFT size and window. Compatible subscribers reuse one branch
and its two channel plans; incompatible sizes or windows create
the smallest separate branch. All four supported sizes may run concurrently.
Plans, transform buffers, channel buffers, and result buffers persist between
frames and are released immediately after the last subscriber leaves. Each
branch records processed frame identity and input fingerprint, so compatible
subscribers read one cached result rather than repeating channel transforms for
the same frame. Conflicting identities and out-of-order frames are rejected.
Overlay exposes shared L/R arrays; difference is `L - R`; balance is
`(L - R) / (L + R)` and is unavailable for near-zero denominators. Unknown
subscriptions, unavailable results, wrong interleaved frame lengths, zero sample
rates, and non-finite samples return typed errors without replacing the last
valid result. Published analysis includes stream ID, frame ID, sample rate, and
frequency-bin mapping.

Raw complex bins remain private to Rust and only feed phase-sensitive downstream
transforms. Existing logarithmic, linear, and musical analyzers retain their
single-channel compatibility path.

## Smoothing, averages, and holds

For a target x, previous value y, elapsed time dt, and time constant tau:

```text
alpha = 1 - exp(-dt / tau)
y_new = y + alpha * (x - y)
```

Attack and release are independent. Defaults are 5 ms attack and 120 ms release.
Users may set 0.5–5,000 ms within Expert bounds. Short and long averages are
separate exponential histories; default durations are 1 s and 10 s. Peak hold
stores the maximum with configurable 0–30 s decay or infinite hold. Reset rules
come from the session lifecycle, not the renderer.

## Configurable bands

Default boundaries (Hz) are:

| Band | Range |
|---|---:|
| Infrabass | 20–40 |
| Subbass | 40–120 |
| Bass | 120–250 |
| Low-mid | 250–500 |
| Mid | 500–2,000 |
| High-mid | 2,000–4,000 |
| Presence | 4,000–8,000 |
| Brilliance | 8,000–16,000 |
| Air | 16,000–Nyquist |

Users may add, remove, rename, reorder, overlap, or leave gaps between bands.
The editor always displays those gaps and overlaps.

Selectable calculations:

- **Bin power:** sum corrected bin powers whose centers fall in the band.
- **Integrated PSD:** integrate PSD over bin bandwidth.
- **Filtered RMS:** apply a documented bandpass bank then compute RMS.
- **Relative energy:** band energy divided by the selected total-energy denominator.

Each result records method, channel mode, bounds, normalization, and units.
L/R-independent, linked L/R, Mid/Side, and mono-compatible modes are distinct
product requests.

## Colored waveform

The waveform is divided into temporal cells selected by zoom level. Each cell
stores peak, RMS, coverage, and optional spectral descriptors. Dominant frequency
is the frequency of the maximum smoothed band energy, with a confidence value
based on peak-to-neighbor ratio.

Color modes:

- dominant-frequency hue on a logarithmic 20 Hz–Nyquist scale;
- fixed band palette using the active band profile;
- RGB energy mixing from low/mid/high groups;
- energy as brightness or alpha;
- phase-risk hatch/outline when mono correlation is below the configured threshold.

Unknown, stale, interpolated, and incomplete cells are distinct from measured
silence. The shape remains legible in monochrome.

## Spectrogram

A row is:

```text
SpectrogramRow {
  session_id
  sequence
  timestamp_samples
  fft_size
  frequency_min
  frequency_max
  dynamic_range_db
  values_u8_or_f32
  palette_id
  validity
}
```

Rows reuse Spectrum products. The renderer owns a bounded ring of rows. Default
history is 30 seconds; Expert may select 5 seconds to 10 minutes subject to a
memory budget. Palette and dynamic range are display transforms.

## Loudness and true peak

Loudness uses ITU-R BS.1770 / EBU R128 K-weighting, channel weighting, absolute
gating, relative gating, LUFS-M, LUFS-S, LUFS-I, and LRA. The implementation
publishes the exact coefficient set and block durations with its calibration
results. Pause freezes the integration clock. Gaps invalidate affected
integrated results.

True peak uses a 4x oversampled polyphase reconstruction by default. Expert may
select 8x if the measured CPU budget allows it. The product reports per-channel
and maximum dBTP, oversampling factor, filter version, and validity. Sample peak
and true peak are never conflated.

## Stereo and phase

Broadband correlation is Pearson correlation over a configurable short window,
with zero-variance frames marked unavailable. Width defaults to
20 log10(RMS(Side)/RMS(Mid)), with explicit handling near zero.

The goniometer plots Mid on X and Side on Y. Frequency stereo derives the same
quantities from per-channel complex FFT products. A configurable visual warning
band defaults to correlation below -0.2; it is a display threshold, not a
quality judgment. Mono fold-down is computed from the normalized Mono signal and
can be auditioned or displayed.

## Experimental decision products

All experimental products expose their formula, window, baseline, and version:

- occupancy percentile fields;
- transient/sustain ratio;
- mono survivability by frequency;
- density/crest-factor map;
- snapshot-to-snapshot coexistence matrix.

They may be added as tiles or overlays without adding a new FFT calculation when
their product key is compatible.
