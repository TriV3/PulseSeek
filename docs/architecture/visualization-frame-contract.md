# Visualization Frame Contract

PR-081 defines the internal real-time boundary used by later visualization and
analysis workers. It covers FR-VS-008 and FR-VS-009 without implementing FFTs,
renderers, or a plugin ABI.

## Frame

`VisualizationFrame` is an immutable value containing a sequence number,
playback-frame position, sample rate, channel count, and interleaved `f32`
samples. The payload uses fixed storage for at most 2,048 samples. Its
constructor rejects empty, oversized, zero-rate, zero-channel, and incomplete
interleaved frames.

This is an internal Rust contract. The separately planned visualizer plugin API
owns external versioning and compatibility rules.

## Callback boundary

`VisualizationTap` accumulates post-volume output samples in fixed storage. It
is configured and attached before `PlaybackConsumer` enters the audio callback.
When a frame is complete, the tap moves it into the existing `ringbuf` SPSC
queue with `try_publish`.

The tap channel count must match the callback output layout. Samples from a
different layout are ignored instead of being published with incorrect frame
metadata, and any partial visualization frame is discarded. Playback output is
unaffected.

The callback path:

- allocates no heap memory;
- acquires no locks;
- performs no I/O, logging, FFT, rendering, or subscriber management;
- executes bounded copy and atomic operations only.

With no tap attached, playback follows the existing path. Filling or closing a
visualization queue cannot pause, fail, or otherwise alter playback output.

## Capacity and drop policy

Channel capacity is fixed when the publisher and subscriber are created and
must be greater than zero. Publication never waits. When the queue is full, the
incoming visualization frame is dropped and a monotonic counter is incremented.
The subscriber can drain queued frames and inspect the counter to detect lag.

This policy deliberately gives playback priority. A later analysis worker may
discard additional stale queued frames before expensive processing.

## Lifecycle

The channel has one callback-owned publisher and one worker-owned subscriber.
Dropping the subscriber makes subsequent publication return `SubscriberGone`.
Dropping the publisher lets the subscriber drain buffered frames before it
reports closed. Explicit shutdown is shared atomically and makes later
publication return `Shutdown`.

Subscriber registration and removal do not occur on the audio callback. A new
playback session creates a new channel when visualization processing is active.

## Exclusions

PR-081 does not add a Tauri command, React state, rendering, FFT processing,
cache/database records, filesystem access, plugin loading, or a third-party
dependency.
