# Player analysis taps

PulseSeek playback exposes two optional bounded capture points.

- Source captures decoded mono or stereo PCM before sample-rate conversion, gain, channel mapping, and output. Blocks retain decoder sample rate and channel order.
- Monitor captures resampled mono or stereo PCM before seek-ramp presentation, user volume, output channel mapping, and output. Blocks retain output sample rate and channel order.

Both points publish versioned `AnalysisBlock` metadata through preallocated bounded capture channels. Source work runs on playback worker. Monitor work runs in audio callback and performs bounded stack copying, atomic updates, and lock-free queue publication only. Callback publication and session rotation clone only `Arc` handles and integer generations; String-backed identities are materialized by consumer off callback. Missing consumers, shutdown, invalid blocks, and full queues drop analysis data without changing or delaying playback. Queue saturation marks next accepted block discontinuous.

Normal seek rotates Source and Monitor session identity, resets block sequence, and preserves seek-relative source counters across resampler reset. Loop wraps retain session identity. Native playback composition creates fresh capture producers for direct file changes. Sequential preparation retains decoded Source PCM while priming; promotion publishes it under rotated Source identity, resets sequence/counters, and updates Source format to prepared decoder rate. Monitor callback publication splits exactly at consumed track marker and rotates identity before publishing new-track frames. Output callbacks exceeding capture-block capacity flush contiguous bounded blocks and continue with advanced first-sample counters. Consumers derive session identity from immutable configuration plus captured generation, so already queued blocks retain original identity across session rotation.

External capture, DSP products, meter rendering, and complete transport-product lifecycle remain separate work.
