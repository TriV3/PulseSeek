# FFT Worker

PR-082 provides the off-thread frequency analysis used by the logarithmic,
linear, and musical visualizers planned in PR-083 through PR-085. It consumes
the bounded time-domain frames introduced by PR-081 and does not add a Tauri or
React boundary.

## Spectrum contract

`SpectrumFrame` is an immutable technical value. It preserves the input frame's
sequence number, playback-frame position, and sample rate, and adds the FFT
size and one-sided magnitude bins. An FFT of `N` real samples produces
`N / 2 + 1` bins from DC through Nyquist. A bin's centre frequency is
`bin_index * sample_rate / N`.

The contract accepts only finite, non-negative magnitudes, a positive sample
rate, a power-of-two FFT size, and the exact corresponding bin count. Spectrum
frames are transient analysis data: they are not manager records, database
rows, or filesystem cache entries.

## Analysis

`FftAnalyzer` accepts one configured power-of-two frame size. Interleaved
channels are averaged to mono, then a periodic Hann window is applied. The
real-to-complex transform runs through `realfft` on the analysis worker, never
on the audio callback. Transform input, output, and scratch buffers are reused
between frames.

Magnitudes are normalized by the Hann window's coherent gain. Interior
positive-frequency bins use the one-sided factor of two; DC and Nyquist do not.
Consequently, a bin-centred sine wave reports its source peak amplitude within
floating-point tolerance. The logarithmic analyzer integration uses 4,096 audio
frames for stereo output (8,192 interleaved samples), although the analyzer
supports other valid power-of-two sizes up to the fixed input frame capacity.

## Worker lifecycle and lag

`FftWorker` owns a named thread, one `VisualizationSubscriber`, and one analyzer.
Before each transform it drains all currently queued inputs and keeps only the
newest frame. Its monotonic skipped-input counter makes that lag visible. This
preserves the playback-first policy: stale visualization work is discarded
instead of being allowed to accumulate.

Results use a bounded synchronous channel with non-blocking `try_send`. A full
result channel drops the incoming spectrum and increments a second counter.
Disconnecting the result receiver ends the worker normally. Explicit
cancellation is an atomic request observed before input polling and before each
transform; waiting returns `Cancelled`. Dropping the worker also requests
cancellation and joins the thread.

## Failure modes and exclusions

Invalid FFT sizes, input-size mismatches, invalid output capacity, transform
failure, worker startup failure, cancellation, and worker panic are represented
by typed `FftError` variants. No failure is sent to or handled by the audio
callback.

PR-082 does not attach the worker to a Tauri playback session, serialize
spectrum frames, render a visualizer, persist settings, group bins into musical
pitches, or add a plugin API. Those integrations remain owned by later PRs.
