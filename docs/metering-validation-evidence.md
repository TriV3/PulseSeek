# PR-092 validation evidence

PR-092 documentation validation followed Red → Green:

1. Red: `PULSESEEK_VALIDATION_ROOT=/tmp/empty node scripts/validate-metering-specs.mjs`
   failed with a missing traceability file. A second Red run against the working
   tree failed with `Canonical default tile list missing` before reconciliation.
2. Green: after specification reconciliation, `node scripts/validate-metering-specs.mjs`
   passed, validating 152 requirements, 152 explicit matrix rows, 26
   cross-document links, rates, source points, default tiles, versions, and loss
   semantics. `pnpm test:metering-specs` was unavailable because pnpm was not
   installed in verification environment.

The validator is `scripts/validate-metering-specs.mjs`. It must reject duplicate
requirement IDs, missing requirement IDs, missing explicit matrix rows, broken
Markdown references, inconsistent versions, incomplete rate coverage, stale
source-point vocabulary, stale loss semantics, and missing canonical defaults.

# PR-101 validation evidence

PR-101 shared FFT-bank behavior followed Red → Green:

1. Red: after adding `crates/pulseseek-playback/tests/fft_bank.rs`,
   `cargo test -p pulseseek-playback --test fft_bank` failed with six compiler
   errors: missing `FftBank`, `FftBranchKey`, and `FftBankAnalysis`; missing
   `ChannelMode::EnergySum`; and missing typed frame-size and unknown-subscription
   errors.
2. Red: after adding the channel-mode contract test,
   `cargo test -p pulseseek-domain --test analysis_subscriptions
   spectrum_channel_modes_include_shared_fft_products` failed with three
   compiler errors for missing `EnergySum`, `LeftRightOverlay`, and
   `LeftRightBalance` variants.
3. Green: after implementing channel contracts and FFT bank,
   `cargo test -p pulseseek-playback --test fft_bank` passed all twelve tests and
   `cargo test -p pulseseek-domain --test analysis_subscriptions` passed all six
   tests.
4. Regression: focused FFT kernel, worker, musical-spectrum, visualization, and
   calibration suites passed 37 tests. Full Rust workspace passed 986 tests;
   frontend passed 740 tests and production build.
