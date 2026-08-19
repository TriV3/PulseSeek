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
