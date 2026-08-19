# Metering event contract validation

**Scope:** PR-102 analysis event contracts

## TDD evidence ledger

Implementation followed Red → Green order in existing checkout. First focused test invocation preceded production module creation and failed at compile time because both public modules and exported types were absent. Production modules were then added. Subsequent focused runs exposed and fixed type mismatch, ordering assertion, delimiter, temporary-borrow, and export collisions before passing.

The initial Red invocation was:

```text
cargo test -p pulseseek-domain --test analysis_events
error[E0432]: could not find `analysis_events` in `pulseseek_domain`

cargo test -p pulseseek-playback --test analysis_event_runtime_contract
error[E0432]: unresolved imports `AnalysisEventRuntime`, `EventEnvelope`, and event contract types
```

This ledger records test-first execution; it does not rely on commit history.

## Green evidence

```text
cargo test -p pulseseek-domain --test analysis_events
6 passed; 0 failed
cargo test -p pulseseek-playback --test analysis_event_runtime_contract
8 passed; 0 failed
```

Coverage includes schema rejection, all ten family names and policies, metadata ordering and timestamps, session ordering reset, validity, experimental metadata, bounded independent queues, latest-only replacement, continuous gap/incomplete signaling, family isolation, cadence policy presence and enforcement, runtime schema compatibility, receiver drop, explicit unsubscribe, and repeated unsubscribe.

## Full validation

`graphify update .` completed successfully. `cargo fmt --all --check`, workspace Clippy with warnings denied, workspace tests, frontend format/lint/typecheck/tests/build, and `pnpm test:metering-specs` passed. Metering validation reported 152 requirements and 26 documentation links.
