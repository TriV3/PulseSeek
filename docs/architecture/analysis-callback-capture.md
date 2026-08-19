# Analysis Callback Capture

PR-096 defines bounded SPSC transport from an audio callback to future analysis workers. It implements the capture boundary only; DSP execution lanes, concrete capture taps, Tauri events, and rendering remain excluded.

## Callback boundary

`AnalysisCaptureProducer::try_capture` accepts one complete mono or stereo interleaved block. Channel layout is validated when `AnalysisCaptureConfig` is built. Each accepted block preserves source, session, measurement point, format, sample position, frame count, sequence, samples, and discontinuity state.

Queue and shared state are created before callback use. Callback publication uses fixed stack storage, bounded sample copying, lock-free `ringbuf` publication, and atomics. It performs no heap allocation, locking, logging, I/O, SQL, DSP, Tauri/React communication, or subscription management. Conversion to allocating `AnalysisBlock` storage happens only when consumer receives block off callback.

## Saturation and validity

Capacity is fixed and positive. Publication never waits. Full queue drops incoming block, increments monotonic dropped-block and dropped-frame counters, advances sequence, and marks next accepted block discontinuous. Future continuous-analysis lanes can therefore detect missing work rather than silently treating results as complete.

Invalid, empty, non-finite, oversized, or incomplete interleaved blocks are rejected without publication or saturation-counter changes.

## Lifecycle

Producer and consumer share atomic shutdown state. Explicit shutdown rejects later capture. Dropping consumer makes producer report `ConsumerGone`. Dropping producer permits consumer to drain queued blocks before reporting closed.

## TDD evidence

Contract test was written and run before production module existed. Expected Red failure was unresolved imports for `analysis_capture_channel`, `AnalysisCaptureConfig`, `CaptureResult`, and `MAX_ANALYSIS_CAPTURE_SAMPLES`. After implementation, focused suite passed all eight tests covering capacity/FIFO, metadata and samples, overflow counters and discontinuity, playback-input invariance, stalled-consumer non-blocking behavior, shutdown, invalid blocks, and channel-layout validation.
