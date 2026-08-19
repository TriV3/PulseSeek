# Analysis Execution Lanes

PR-097 defines bounded worker execution between callback capture and future DSP products. Player taps, DSP algorithms, product-graph orchestration, Tauri events, adaptive degradation, and UI remain excluded.

## Ownership

Each lane owns one named worker. Continuous and latest-only visual work use separate lanes; products and tiles never create workers through this API. Processors run off audio and UI threads. Callers select positive input and output capacities when constructing a lane.

## Backpressure and validity

Submission uses bounded `try_send` and never waits. Visual input or output saturation drops work and increments visual-drop counters. Visual workers drain queued stale inputs before processing newest available work. Visual loss never changes continuous validity.

Continuous input or output saturation increments continuous-gap counters and permanently marks subsequent outputs incomplete for that lane lifetime. Explicit block discontinuities and sequence gaps do the same. Existing queued output may predate loss; consumers must treat every later incomplete output as authoritative.

Diagnostics snapshots expose current and high-water input/output depth, visual drops, stale visual inputs, continuous gaps, and processor panics. Later diagnostics and degradation PRs may publish and act on these values without adding callback logging.

## Failure and shutdown

Dropping last output receiver makes idle worker exit. Explicit sender shutdown also stops worker. Dropped or disconnected endpoints return typed submission results. Processor panics are caught at work-item boundary, counted, and mark continuous output incomplete; worker continues and sibling lanes remain independent. Unexpected infrastructure-thread panic is reported by worker join.

## Real-time boundary

Lane producers allocate `AnalysisBlock` before submission and use standard bounded channels. They are application-worker infrastructure, not callback-safe capture. Audio callback continues using preallocated SPSC transport from PR-096, and future player taps must never invoke lane construction, waiting, processing, or diagnostics from callback.

## TDD evidence

Contract tests were run before module implementation and failed on unresolved lane API imports. Green tests cover latest-only visual drops and stale-input handling, continuous saturation and discontinuity validity, bounded nonblocking backpressure, idle receiver shutdown, panic isolation, and invalid capacities.
