# PulseSeek Pull-Request Implementation Plan

## 1. Purpose

This plan turns the PulseSeek roadmap into small, ordered Pull Requests.

There are no disposable investigation branches. Every branch must produce
tested, maintainable code or versioned project documentation that can be merged
into `develop`.

The accepted stack is not under evaluation:

- Tauri 2
- React
- Strict TypeScript
- Rust core
- `cpal`, `symphonia`, `rubato`, and `lofty`
- `rusqlite`
- Vitest, React Testing Library, Playwright, `rstest`, and `proptest`

## 2. Pull Request rules

### 2.1 One PR, one outcome

Each PR must deliver exactly one of:

- One project capability
- One domain contract
- One concrete adapter
- One user-visible behavior
- One isolated infrastructure improvement
- One database migration
- One external integration operation

A PR must not combine a refactor with an unrelated feature.

### 2.2 Size target

- Prefer fewer than 400 changed lines of production logic.
- Generated lockfiles and generated Tauri files do not count toward this target.
- Split a PR when it introduces more than one independently testable behavior.
- A PR may prepare an interface without providing every future implementation.

### 2.3 Required PR description

Every PR must state:

- **Outcome**
- **Dependencies**
- **Requirements covered**
- **Tests added first**
- **Manual verification**
- **Out of scope**
- **Risks**

### 2.4 Branch names

```text
docs/pr-001-project-governance
chore/pr-002-cargo-workspace
chore/pr-003-tauri-react-shell
```

The branch prefix describes the nature of the PR:

- `feature/` for application capabilities, domain contracts, adapters, and
  user-visible behavior.
- `chore/` for repository scaffolding and maintenance configuration.
- `docs/` for documentation-only outcomes.
- `test/` for test infrastructure and non-production test harnesses.
- `ci/` for continuous-integration workflows.
- `build/` for packaging and distributable artifacts.

All implementation PRs target `develop`. No direct push is assumed by this
plan.

### 2.5 TDD sequence inside a PR

1. Add a failing test for the PR outcome.
2. Confirm the expected failure.
3. Implement the minimum behavior.
4. Run the focused test.
5. Run all affected suites.
6. Refactor without expanding scope.

Infrastructure-only PRs must add a verification command or CI assertion when a
behavior test is not applicable.

## 3. Dependency notation

- `—` means the PR has no implementation dependency.
- `PR-004` means that PR must already be merged.
- Multiple dependencies are comma-separated.
- PR numbers define the recommended merge order, but independent PRs may be
  developed concurrently after their dependencies are merged.

## 4. Milestone A — Repository and application shell

### PR-001 — Version project governance

- **Branch:** `docs/pr-001-project-governance`
- **Depends on:** —
- **Outcome:** Add `AGENTS.md`, architecture documents, ADRs, MPL-2.0 notice,
  Rust toolchain pin, and updated README.
- **Tests/verification:** Markdown whitespace check; verify internal links and
  `rustc --version`.
- **Acceptance:** A new agent can discover the accepted stack, TDD rules,
  authority boundaries, and Git workflow from tracked files.
- **Out of scope:** Application code and CI.

### PR-002 — Create Cargo workspace

- **Branch:** `chore/pr-002-cargo-workspace`
- **Depends on:** PR-001
- **Outcome:** Create the root Cargo workspace and the first
  `pulseseek-domain` library crate.
- **Tests first:** A minimal domain crate test proving workspace test discovery.
- **Acceptance:** `cargo test --workspace` succeeds.
- **Out of scope:** Tauri, audio dependencies, and business behavior.

### PR-003 — Create Tauri and React shell

- **Branch:** `chore/pr-003-tauri-react-shell`
- **Depends on:** PR-002
- **Outcome:** Add the Tauri 2 desktop app, React, strict TypeScript, Vite, and
  pnpm lockfile.
- **Tests/verification:** Frontend smoke test renders the application name;
  Tauri and frontend production builds succeed.
- **Acceptance:** The desktop shell opens and displays `PulseSeek`.
- **Out of scope:** Navigation, themes, filesystem, and audio.

### PR-004 — Configure Rust quality gates

- **Branch:** `chore/pr-004-rust-quality`
- **Depends on:** PR-002
- **Outcome:** Configure rustfmt, Clippy warnings-as-errors, and `cargo-deny`.
- **Tests/verification:** All configured commands run locally.
- **Acceptance:** Deliberate formatting or lint failure is detected.
- **Out of scope:** GitHub Actions and frontend quality.

### PR-005 — Configure frontend quality gates

- **Branch:** `chore/pr-005-frontend-quality`
- **Depends on:** PR-003
- **Outcome:** Configure ESLint, Prettier, strict type checking, and package
  scripts.
- **Tests/verification:** Formatting, lint, and type-check scripts succeed.
- **Acceptance:** Deliberate lint and type errors are detected.
- **Out of scope:** Unit tests and CI.

### PR-006 — Configure frontend test harness

- **Branch:** `test/pr-006-frontend-tests`
- **Depends on:** PR-003
- **Outcome:** Configure Vitest, React Testing Library, DOM test setup, and typed
  Tauri port fakes.
- **Tests first:** Render the shell and assert its accessible application name.
- **Acceptance:** `pnpm test` runs the frontend suite.
- **Out of scope:** Playwright and production components.

### PR-007 — Add initial GitHub Actions checks

- **Branch:** `ci/pr-007-ci`
- **Depends on:** PR-004, PR-005, PR-006
- **Outcome:** Run Rust formatting, Clippy, Rust tests, frontend formatting,
  lint, types, tests, and builds on PRs.
- **Tests/verification:** Validate workflow syntax and observe one successful
  run on the PR.
- **Acceptance:** Every configured job reports a stable GitHub check name.
- **Out of scope:** Required-check branch rules and release builds.

### PR-008 — Add application error contract

- **Branch:** `feature/pr-008-error-contract`
- **Depends on:** PR-002
- **Outcome:** Define typed domain/application errors and safe user-facing error
  descriptors.
- **Tests first:** Verify error category, safe message, diagnostic context, and
  absence of raw private paths.
- **Acceptance:** Adapters can return typed errors without depending on the UI.
- **Out of scope:** Logging and error notifications.

### PR-009 — Add structured local diagnostics

- **Branch:** `feature/pr-009-local-diagnostics`
- **Depends on:** PR-003, PR-008
- **Outcome:** Initialize bounded rotating logs with `tracing`.
- **Tests first:** Verify redaction and bounded log-file configuration.
- **Acceptance:** Rust and React boundary errors produce local redacted
  diagnostics.
- **Out of scope:** Diagnostic export UI and telemetry.

## 5. Milestone B — Playback domain

### PR-010 — Define playback state

- **Branch:** `feature/pr-010-playback-state`
- **Depends on:** PR-002
- **Requirements:** FR-AU-003
- **Outcome:** Define `Stopped`, `Loading`, `Playing`, `Paused`, and `Failed`
  states with valid transitions.
- **Tests first:** Table-driven transition tests, including invalid commands.
- **Acceptance:** State changes are deterministic and contain no audio-library
  dependency.
- **Out of scope:** Decoding and audio output.

### PR-011 — Define playback position and duration

- **Branch:** `feature/pr-011-playback-position`
- **Depends on:** PR-010
- **Requirements:** FR-AU-003
- **Outcome:** Add validated position, duration, and seek-target value types.
- **Tests first:** Boundary and property tests for negative, overflowing, and
  unknown durations.
- **Acceptance:** Invalid seek values cannot enter the playback state.
- **Out of scope:** Performing a decoder seek.

### PR-012 — Define playback modes

- **Branch:** `feature/pr-012-playback-modes`
- **Depends on:** PR-010
- **Requirements:** FR-AU-006, FR-AU-007, FR-AU-008
- **Outcome:** Define one-shot, loop-current, sequential, and random modes.
- **Tests first:** End-of-file decisions for every mode.
- **Acceptance:** The domain returns the next action without knowing the audio
  backend.
- **Out of scope:** Seamless audio looping and random selection algorithm.

### PR-013 — Define playback queue navigation

- **Branch:** `feature/pr-013-queue-navigation`
- **Depends on:** PR-012
- **Requirements:** FR-AU-005
- **Outcome:** Define previous/next behavior for an ordered playable-file list.
- **Tests first:** Empty, single-item, first, middle, last, and removed-item
  cases.
- **Acceptance:** Navigation behavior is independent of React and the
  filesystem.
- **Out of scope:** Folder enumeration.

### PR-014 — Define volume model

- **Branch:** `feature/pr-014-volume-model`
- **Depends on:** PR-010
- **Requirements:** FR-AU-004
- **Outcome:** Add clamped linear gain and mute state.
- **Tests first:** Boundary, mute/unmute, and round-trip tests.
- **Acceptance:** Invalid gain cannot reach the audio adapter.
- **Out of scope:** Applying gain to samples.

## 6. Milestone C — Decoder capability

### PR-015 — Define decoder port

- **Branch:** `feature/pr-015-decoder-port`
- **Depends on:** PR-008, PR-011
- **Requirements:** FR-BR-003, FR-BR-004
- **Outcome:** Define decoder capability probing, stream metadata, read, and
  seek ports.
- **Tests first:** Contract test using a handwritten fake decoder.
- **Acceptance:** The domain depends only on the decoder port.
- **Out of scope:** Symphonia and file extensions.

### PR-016 — Implement WAV decoding

- **Branch:** `feature/pr-016-wav-decoder`
- **Depends on:** PR-015
- **Requirements:** Supported formats
- **Outcome:** Decode and seek supported WAV fixtures with Symphonia.
- **Tests first:** Valid PCM WAV, unsupported encoding, corrupt header, seek,
  mono, and stereo fixtures.
- **Acceptance:** Decoded frames and metadata match fixtures.
- **Out of scope:** FLAC and MP3.

### PR-017 — Implement FLAC decoding

- **Branch:** `feature/pr-017-flac-decoder`
- **Depends on:** PR-016
- **Requirements:** Supported formats
- **Outcome:** Decode and seek FLAC through the same adapter.
- **Tests first:** Valid, corrupt, mono/stereo, and seek fixtures.
- **Acceptance:** FLAC satisfies the decoder contract suite.
- **Out of scope:** MP3.

### PR-018 — Implement MP3 decoding

- **Branch:** `feature/pr-018-mp3-decoder`
- **Depends on:** PR-016
- **Requirements:** Supported formats
- **Outcome:** Decode and seek MP3 through the same adapter.
- **Tests first:** Valid, corrupt, variable-bitrate, and seek fixtures.
- **Acceptance:** MP3 satisfies the decoder contract suite.
- **Out of scope:** Gapless album playback.

### PR-019 — Implement decoder registry

- **Branch:** `feature/pr-019-decoder-registry`
- **Depends on:** PR-016, PR-017, PR-018
- **Requirements:** FR-BR-003, FR-BR-004
- **Outcome:** Select a decoder from actual probe results.
- **Tests first:** Supported content with misleading extension, unsupported
  content with audio extension, corrupt files, and registry ordering.
- **Acceptance:** Capability is not decided from extension alone.
- **Out of scope:** UI filtering.

### PR-020 — Read basic audio metadata

- **Branch:** `feature/pr-020-basic-metadata`
- **Depends on:** PR-019
- **Requirements:** FR-LS-002
- **Outcome:** Read duration, channels, sample rate, bit depth, and codec.
- **Tests first:** Metadata fixtures across initial formats.
- **Acceptance:** Missing optional metadata does not reject a playable file.
- **Out of scope:** Artist, album, BPM, and key.

## 7. Milestone D — Audio output

### PR-021 — Define audio output port

- **Branch:** `feature/pr-021-audio-output-port`
- **Depends on:** PR-008, PR-014
- **Requirements:** FR-IO-001–FR-IO-004
- **Outcome:** Define device enumeration, selection, stream start/stop, and
  device-loss events.
- **Tests first:** Contract tests with a fake output adapter.
- **Acceptance:** Playback services do not depend directly on `cpal`.
- **Out of scope:** Real hardware.

### PR-022 — Enumerate output devices

- **Branch:** `feature/pr-022-enumerate-devices`
- **Depends on:** PR-021
- **Requirements:** FR-IO-001, FR-IO-003
- **Outcome:** Implement cpal device enumeration and default-device resolution.
- **Tests first:** Adapter mapping tests plus guarded hardware integration test.
- **Acceptance:** Available devices have stable session identifiers and display
  names.
- **Out of scope:** Selecting a device.

### PR-023 — Select output device

- **Branch:** `feature/pr-023-select-device`
- **Depends on:** PR-022
- **Requirements:** FR-IO-002, FR-IO-003
- **Outcome:** Open the chosen output and fall back to the system default.
- **Tests first:** Selection success, unavailable device, and default fallback.
- **Acceptance:** The selected device can accept a test stream.
- **Out of scope:** User interface and persistence.

### PR-024 — Stream decoded frames

- **Branch:** `feature/pr-024-stream-decoded-frames`
- **Depends on:** PR-019, PR-023
- **Requirements:** FR-AU-001, FR-AU-002
- **Outcome:** Connect decoder worker output to the real-time output buffer.
- **Tests first:** Fake decoder/output integration tests for ordering,
  starvation, and shutdown.
- **Acceptance:** A fixture plays from start to end without work prohibited on
  the callback.
- **Out of scope:** Pause, seek, resampling, and looping.

### PR-025 — Apply volume and mute

- **Branch:** `feature/pr-025-apply-volume`
- **Depends on:** PR-024
- **Requirements:** FR-AU-004
- **Outcome:** Apply gain and mute in the audio signal path.
- **Tests first:** Sample-value tests for unity, attenuation, mute, and clipping
  policy.
- **Acceptance:** Volume changes do not restart decoding.
- **Out of scope:** UI slider.

### PR-026 — Add pause and resume

- **Branch:** `feature/pr-026-pause-resume`
- **Depends on:** PR-024, PR-010
- **Requirements:** FR-AU-003
- **Outcome:** Pause output while preserving the playback position and resume.
- **Tests first:** Pause, repeated pause, resume, and end-while-paused cases.
- **Acceptance:** Resume continues from the preserved position.
- **Out of scope:** Stop and seek.

### PR-027 — Add stop

- **Branch:** `feature/pr-027-stop-playback`
- **Depends on:** PR-026
- **Requirements:** FR-AU-003
- **Outcome:** Stop playback, release resources, and reset position.
- **Tests first:** Stop from every playback state.
- **Acceptance:** Stop is idempotent and returns to position zero.
- **Out of scope:** Automatic next item.

### PR-028 — Add seek

- **Branch:** `feature/pr-028-seek-playback`
- **Depends on:** PR-024, PR-011
- **Requirements:** FR-AU-003
- **Outcome:** Seek the decoder and refill the output buffer safely.
- **Tests first:** Seek while playing/paused, seek to bounds, unsupported seek,
  and rapid repeated seek.
- **Acceptance:** Playback resumes at the validated target without stale frames.
- **Out of scope:** Waveform interaction.

### PR-029 — Add sample-rate conversion

- **Branch:** `feature/pr-029-resampling`
- **Depends on:** PR-024
- **Requirements:** FR-IO-005
- **Outcome:** Resample mismatched decoder and device rates with Rubato.
- **Tests first:** Known signal duration and sample-count conversions.
- **Acceptance:** A mismatched-rate fixture plays at the correct speed.
- **Out of scope:** User-selectable quality modes.

### PR-030 — Implement one-shot completion

- **Branch:** `feature/pr-030-one-shot`
- **Depends on:** PR-012, PR-024
- **Requirements:** FR-AU-006
- **Outcome:** Stop cleanly at the end of the current file.
- **Tests first:** Normal end, empty file, corrupt tail, and stop race.
- **Acceptance:** End-of-file emits one completion event and releases playback.
- **Out of scope:** Loop and sequential modes.

### PR-031 — Implement current-file loop

- **Branch:** `feature/pr-031-loop-current`
- **Depends on:** PR-030
- **Requirements:** FR-AU-007
- **Outcome:** Restart the current file at its loop boundary.
- **Tests first:** Multiple cycles, mode change near boundary, and stop during
  loop.
- **Acceptance:** The playback clock remains consistent across cycles.
- **Out of scope:** Seamless single-cycle optimization and A–B repeat.

### PR-032 — Make short loops seamless

- **Branch:** `feature/pr-032-seamless-short-loop`
- **Depends on:** PR-031
- **Requirements:** FR-AU-010
- **Outcome:** Prebuffer short content so loop boundaries do not underrun.
- **Tests first:** Single-cycle and short-sample fixtures over many repetitions.
- **Acceptance:** Automated buffer assertions pass and manual macOS check finds
  no boundary gap on the reference device.
- **Out of scope:** Crossfade loops.

### PR-033 — Recover from output-device loss

- **Branch:** `feature/pr-033-device-loss`
- **Depends on:** PR-023, PR-026
- **Requirements:** FR-IO-004
- **Outcome:** Pause safely and expose recovery when the active device vanishes.
- **Tests first:** Device-loss event, fallback available, no fallback, and
  repeated disconnect.
- **Acceptance:** Device loss never crashes or leaves the state as `Playing`.
- **Out of scope:** Device-selection UI.

## 8. Milestone E — Typed Tauri boundary

### PR-034 — Add versioned command envelope

- **Branch:** `feature/pr-034-command-envelope`
- **Depends on:** PR-003, PR-008
- **Outcome:** Define validated request/response types and boundary error
  mapping.
- **Tests first:** Serialization, unknown version, invalid payload, and safe
  error conversion.
- **Acceptance:** React can call a typed no-op health command.
- **Out of scope:** Playback commands.

### PR-035 — Expose playback commands

- **Branch:** `feature/pr-035-playback-commands`
- **Depends on:** PR-027, PR-028, PR-034
- **Requirements:** FR-AU-003
- **Outcome:** Expose play, pause, resume, stop, seek, and volume commands.
- **Tests first:** Command-handler tests using fake application services.
- **Acceptance:** Commands validate inputs and never expose concrete adapters.
- **Out of scope:** React controls.

### PR-036 — Expose playback state events

- **Branch:** `feature/pr-036-playback-events`
- **Depends on:** PR-035
- **Outcome:** Send versioned state and throttled position events to React.
- **Tests first:** Event ordering, throttling, terminal state, and subscriber
  disposal.
- **Acceptance:** Position events cannot saturate the WebView.
- **Out of scope:** UI rendering.

### PR-037 — Expose audio-device commands

- **Branch:** `feature/pr-037-device-commands`
- **Depends on:** PR-022, PR-023, PR-034
- **Requirements:** FR-IO-001–FR-IO-004
- **Outcome:** Expose list, current device, and select commands plus loss event.
- **Tests first:** Command and event tests with fake device adapter.
- **Acceptance:** React receives stable typed device data.
- **Out of scope:** Device selector component.

## 9. Milestone F — Folder browser

### PR-038 — Define browser entry model

- **Branch:** `feature/pr-038-browser-entry`
- **Depends on:** PR-002, PR-008
- **Requirements:** FR-BR-002
- **Outcome:** Define folder, playable file, hidden unsupported file, and
  inaccessible entry models.
- **Tests first:** Ordering, Unicode names, and safe path display.
- **Acceptance:** Browser entries contain no UI or decoder implementation.
- **Out of scope:** Filesystem enumeration.

### PR-039 — Enumerate one folder

- **Branch:** `feature/pr-039-enumerate-folder`
- **Depends on:** PR-038
- **Requirements:** FR-BR-001, FR-BR-006, FR-BR-013
- **Outcome:** Enumerate direct children through a filesystem port and native
  adapter.
- **Tests first:** Empty, nested, permission denied, symlink, Unicode, and
  disconnected-path cases.
- **Acceptance:** Enumeration has deterministic folder/file ordering.
- **Out of scope:** Recursive browsing and decoder filtering.

### PR-040 — Stream large folder results

- **Branch:** `feature/pr-040-stream-folder-results`
- **Depends on:** PR-039
- **Requirements:** Performance
- **Outcome:** Make enumeration incremental and cancellable.
- **Tests first:** Batch ordering, cancellation, stale request, and 100,000-entry
  synthetic folder.
- **Acceptance:** The first batch arrives without waiting for full enumeration.
- **Out of scope:** UI virtualization.

### PR-041 — Filter unsupported files

- **Branch:** `feature/pr-041-filter-unsupported`
- **Depends on:** PR-019, PR-040
- **Requirements:** FR-BR-003–FR-BR-005
- **Outcome:** Probe candidate files and hide unsupported entries by default.
- **Tests first:** Playable, unsupported, corrupt, misleading extension, and
  show-unsupported preference.
- **Acceptance:** Only confirmed playable files enter the default visible list.
- **Out of scope:** Metadata columns.

### PR-042 — Expose folder picker and enumeration

- **Branch:** `feature/pr-042-folder-commands`
- **Depends on:** PR-034, PR-040, PR-041
- **Requirements:** FR-BR-001, FR-BR-002
- **Outcome:** Add restricted Tauri folder-picker and browser commands/events.
- **Tests first:** Path validation, cancellation, stale request, and denied path.
- **Acceptance:** React can request a folder without general filesystem access.
- **Out of scope:** Browser components.

### PR-043 — Render folder tree

- **Branch:** `feature/pr-043-folder-tree`
- **Depends on:** PR-006, PR-042
- **Requirements:** FR-BR-002, FR-BR-006
- **Outcome:** Add accessible expandable tree navigation.
- **Tests first:** Expand, collapse, select, keyboard navigation, loading, and
  error states.
- **Acceptance:** The user can navigate nested folders without a mouse.
- **Out of scope:** File list.

### PR-044 — Render virtualized file list

- **Branch:** `feature/pr-044-file-list`
- **Depends on:** PR-006, PR-042
- **Requirements:** FR-LS-001, performance
- **Outcome:** Display streamed playable files through TanStack Virtual.
- **Tests first:** Incremental batches, selection, empty/error states, and
  stable row identity.
- **Acceptance:** A 100,000-row synthetic list remains responsive.
- **Out of scope:** Playback on click and metadata columns.

### PR-045 — Add metadata columns

- **Branch:** `feature/pr-045-metadata-columns`
- **Depends on:** PR-020, PR-044
- **Requirements:** FR-LS-002
- **Outcome:** Show available duration, channels, sample rate, bit depth, and
  codec.
- **Tests first:** Partial metadata, loading metadata, and column formatting.
- **Acceptance:** Missing metadata never removes a playable row.
- **Out of scope:** Sorting and custom column layout.

### PR-046 — Start playback on single click

- **Branch:** `feature/pr-046-single-click-playback`
- **Depends on:** PR-035, PR-036, PR-044
- **Requirements:** FR-AU-001, FR-AU-002
- **Outcome:** Connect file-row selection to the play command.
- **Tests first:** Supported click, rapid selection replacement, command error,
  and keyboard selection.
- **Acceptance:** One click starts the selected file and updates visible state.
- **Out of scope:** Double-click behavior and autoplay preference.

### PR-047 — Add player transport controls

- **Branch:** `feature/pr-047-transport-controls`
- **Depends on:** PR-035, PR-036
- **Requirements:** FR-AU-003–FR-AU-005
- **Outcome:** Add play/pause, stop, previous, next, position, seek, and volume
  controls.
- **Tests first:** Accessible labels, enabled states, command dispatch, and
  failure state.
- **Acceptance:** The complete transport works by mouse and keyboard.
- **Out of scope:** Playback-mode selector.

### PR-048 — Add playback-mode selector

- **Branch:** `feature/pr-048-playback-mode-selector`
- **Depends on:** PR-012, PR-035, PR-047
- **Requirements:** FR-AU-006–FR-AU-008
- **Outcome:** Let the user select one-shot, loop, sequential, or random mode.
- **Tests first:** Visible active mode, keyboard operation, and command failure.
- **Acceptance:** Selected mode is reflected by Rust playback state.
- **Out of scope:** A–B repeat.

### PR-049 — Add output-device selector

- **Branch:** `feature/pr-049-device-selector`
- **Depends on:** PR-037
- **Requirements:** FR-IO-001–FR-IO-004
- **Outcome:** Add accessible device selection and device-loss recovery UI over
  the native audio-device service.
- **Tests first:** Default, selection, missing device, loss, retry, and no-device
  states.
- **Acceptance:** The user can select an available output, see the confirmed
  device, and retry after loss without restarting PulseSeek. Rebinding active
  playback to the selected device is completed by PR-049-2.
- **Out of scope:** Buffer-size and exclusive-mode settings.

### PR-049-2 — Wire real playback service

- **Branch:** `feature/pr-049-2-real-playback-wiring`
- **Depends on:** PR-024, PR-025, PR-026, PR-027, PR-028, PR-033, PR-049
- **Requirements:** FR-AU-001–FR-AU-005, FR-IO-002–FR-IO-004
- **Outcome:** Replace the Tauri fake playback service with an application
  service that connects decoder workers, `pulseseek-playback`, and
  `CpalAudioOutput` for real audible playback.
- **Tests first:** WAV fixture playback through a fake output boundary, pause,
  resume, stop, seek, volume, selected-device rebind, fallback device, device
  loss, recovery, and shutdown.
- **Acceptance:** PulseSeek plays supported audio through the selected output
  device; transport commands affect audible playback; device loss pauses safely
  and recovery can resume playback without restarting PulseSeek.
- **Out of scope:** Playback-mode selector behavior, sequential/random
  end-of-file selection, buffer-size settings, and exclusive-mode settings.

### PR-050 — Add move-to-trash application service

- **Branch:** `feature/pr-050-trash-service`
- **Depends on:** PR-008, PR-039
- **Requirements:** FR-FM-001–FR-FM-003
- **Outcome:** Add a filesystem port and safe operating-system trash adapter.
- **Tests first:** Success, cancellation, permission failure, missing file, and
  partial batch failure.
- **Acceptance:** Permanent deletion is not exposed by this service.
- **Out of scope:** UI confirmation.

### PR-051 — Add move-to-trash UI

- **Branch:** `feature/pr-051-trash-ui`
- **Depends on:** PR-042, PR-044, PR-050
- **Requirements:** FR-FM-001–FR-FM-003
- **Outcome:** Add confirmation, command, visible progress, and list refresh.
- **Tests first:** Confirm/cancel, selected target display, success, and partial
  failure.
- **Acceptance:** Trashed rows disappear without affecting unrelated files.
- **Out of scope:** Permanent deletion and undo.

### PR-052 — Add primary keyboard shortcuts

- **Branch:** `feature/pr-052-player-shortcuts`
- **Depends on:** PR-043, PR-047, PR-048, PR-049-2, PR-051
- **Requirements:** FR-KB-001, FR-KB-002
- **Outcome:** Add open, play/pause, previous, next, seek, loop, and trash
  shortcuts.
- **Tests first:** Shortcut dispatch, focused-input exclusion, platform modifier,
  and conflict tests.
- **Acceptance:** The P0 workflow is usable without a mouse.
- **Out of scope:** Shortcut customization.

### PR-053 — Add P0 Playwright journey

- **Branch:** `test/pr-053-p0-e2e`
- **Depends on:** PR-049, PR-051, PR-052
- **Outcome:** Automate folder open, file selection, transport, mode change, and
  trash confirmation with fake platform adapters.
- **Tests/verification:** Playwright journey in CI.
- **Acceptance:** The critical UI-to-Rust command path is regression-tested.
- **Out of scope:** Real hardware audio assertion.

### PR-054 — Add P0 performance harness

- **Branch:** `test/pr-054-p0-performance`
- **Depends on:** PR-046
- **Outcome:** Measure cold start, enumeration, selection-to-play request, and
  large-list responsiveness.
- **Tests/verification:** Produce machine-readable benchmark output and compare
  against documented budgets without flaky hard failure initially.
- **Acceptance:** Baseline measurements are recorded for the reference machine.
- **Out of scope:** Waveform and visualization performance.

### PR-055 — Persist Audio Player preferences

- **Branch:** `feature/pr-055-player-preferences`
- **Depends on:** PR-043, PR-047, PR-048, PR-049-2
- **Requirements:** FR-AU-014–FR-AU-018
- **Outcome:** Persist each confirmed player option immediately and restore the
  browser context, last played file, output device, playback mode, volume,
  mute, and resizable panel dimensions on launch.
- **Tests first:** Default state, atomic immediate writes, stale-write rejection,
  corrupt-state fallback, unavailable paths/devices, browser restoration, and
  explicit exclusion of transport state and seek position.
- **Acceptance:** Relaunching PulseSeek restores every selectable Audio Player
  option and reveals the last played file without autoplay; transport starts
  stopped at zero.
- **Out of scope:** Manager database state, session marks, playback resume, seek
  restoration, and cloud-drive authentication.

## 10. Milestone G — Themes and visual polish

### PR-056 — Add semantic design tokens

- **Branch:** `feature/pr-055-design-tokens`
- **Depends on:** PR-003
- **Requirements:** Theme architecture
- **Outcome:** Define semantic tokens and remove palette colors from feature
  components.
- **Tests first:** Token completeness and component render smoke tests.
- **Acceptance:** Components render without depending on a named palette.
- **Out of scope:** Multiple themes.

### PR-057 — Add dark and light themes

- **Branch:** `feature/pr-056-light-dark-themes`
- **Depends on:** PR-056
- **Requirements:** Theme architecture
- **Outcome:** Add PulseSeek Dark, PulseSeek Light, and system preference.
- **Tests first:** Theme selection, system change, persistence, and no-restart
  switching.
- **Acceptance:** Core screens are readable in both modes.
- **Out of scope:** Midnight Blue and High Contrast.

### PR-058 — Add Midnight Blue theme

- **Branch:** `feature/pr-057-midnight-theme`
- **Depends on:** PR-057
- **Outcome:** Add the complete Midnight Blue token set.
- **Tests first:** Token completeness and screenshot comparison.
- **Acceptance:** No component falls back to another theme unintentionally.
- **Out of scope:** High Contrast.

### PR-059 — Add High Contrast theme

- **Branch:** `feature/pr-058-high-contrast`
- **Depends on:** PR-057
- **Outcome:** Add a high-contrast accessible theme.
- **Tests first:** Token completeness, keyboard focus visibility, and automated
  contrast checks where reliable.
- **Acceptance:** Primary text and controls meet agreed contrast targets.
- **Out of scope:** User-created themes.

## 11. Milestone H — Waveform

### PR-060 — Define waveform model

- **Branch:** `feature/pr-059-waveform-model`
- **Depends on:** PR-011, PR-015
- **Requirements:** FR-VS-001–FR-VS-003
- **Outcome:** Define multiresolution waveform levels and time/pixel mapping.
- **Tests first:** Mapping boundaries and property tests.
- **Acceptance:** The model has no Canvas, React, or database dependency.
- **Out of scope:** Peak extraction.

### PR-061 — Extract waveform peaks

- **Branch:** `feature/pr-060-waveform-extraction`
- **Depends on:** PR-060, PR-019
- **Requirements:** FR-VS-001
- **Outcome:** Generate multiresolution peaks on a cancellable worker.
- **Tests first:** Known fixtures, mono/stereo policy, cancellation, empty and
  corrupt files.
- **Acceptance:** Extraction never runs on the audio callback.
- **Out of scope:** Persistent cache.

### PR-062 — Define technical cache database

- **Branch:** `feature/pr-061-cache-database`
- **Depends on:** PR-002
- **Requirements:** Technical cache
- **Outcome:** Create `app-cache.sqlite`, migrations, repository port, and
  dedicated worker.
- **Tests first:** Fresh migration, repeat migration, rollback, corrupt database,
  and independent startup.
- **Acceptance:** Cache failure does not prevent Audio Player startup.
- **Out of scope:** Waveform records.

### PR-063 — Cache waveform data

- **Branch:** `feature/pr-062-waveform-cache`
- **Depends on:** PR-061, PR-062
- **Requirements:** FR-VS-010
- **Outcome:** Store and retrieve waveform data using a versioned file cache key.
- **Tests first:** Hit, miss, stale timestamp, size change, algorithm version,
  and corrupt cache row.
- **Acceptance:** Cache records never create manager items.
- **Out of scope:** File watcher invalidation.

### PR-064 — Render waveform canvas

- **Branch:** `feature/pr-063-waveform-canvas`
- **Depends on:** PR-006, PR-060, PR-063
- **Requirements:** FR-VS-001, FR-VS-002
- **Outcome:** Render waveform data and playback progress with Canvas 2D.
- **Tests first:** Renderer mapping tests, loading/error states, resize, and
  theme-token consumption.
- **Acceptance:** React does not redraw through high-frequency state updates.
- **Out of scope:** Seek interaction and style variants.

### PR-065 — Seek from waveform

- **Branch:** `feature/pr-064-waveform-seek`
- **Depends on:** PR-028, PR-035, PR-064
- **Requirements:** FR-VS-003
- **Outcome:** Convert pointer/keyboard interaction into validated seek commands.
- **Tests first:** Click, drag, keyboard, bounds, unavailable duration, and
  command failure.
- **Acceptance:** Visual progress reconciles with confirmed Rust position.
- **Out of scope:** A–B selection.

### PR-066 — Add waveform styles

- **Branch:** `feature/pr-065-waveform-styles`
- **Depends on:** PR-064
- **Requirements:** FR-VS-004
- **Outcome:** Add solid, gradient, and outline renderer styles.
- **Tests first:** Renderer selection, theme compatibility, and screenshots.
- **Acceptance:** Style changes do not regenerate waveform data.
- **Out of scope:** User-authored renderer plugins.

## 12. Milestone I — Advanced browser

### PR-067 — Add file sorting

- **Branch:** `feature/pr-066-file-sorting`
- **Depends on:** PR-045
- **Requirements:** FR-LS-003
- **Outcome:** Sort by name, duration, size, type, date, and path.
- **Tests first:** Stable sorting, missing metadata, locale/Unicode, and
  ascending/descending.
- **Acceptance:** Selection survives sort changes.
- **Out of scope:** Filtering.

### PR-068 — Add folder search

- **Branch:** `feature/pr-067-folder-search`
- **Depends on:** PR-044
- **Requirements:** FR-LS-004
- **Outcome:** Filter the current visible folder by a text query.
- **Tests first:** Case, Unicode, empty query, rapid query, and selection.
- **Acceptance:** Search does not re-enumerate the filesystem.
- **Out of scope:** Recursive search and manager search syntax.

### PR-069 — Add format filters

- **Branch:** `feature/pr-068-format-filter`
- **Depends on:** PR-041, PR-044
- **Requirements:** FR-LS-004
- **Outcome:** Filter visible playable files by decoded format.
- **Tests first:** Single/multiple filters, unknown codec, reset, and selection.
- **Acceptance:** Filters operate on decoder capability, not extension.
- **Out of scope:** Saving filter presets.

### PR-070 — Add multiple selection

- **Branch:** `feature/pr-069-multi-selection`
- **Depends on:** PR-044
- **Requirements:** FR-LS-005
- **Outcome:** Add keyboard and pointer multi-selection.
- **Tests first:** Toggle, range, select all, streamed rows, and removed rows.
- **Acceptance:** Selection uses stable file identity.
- **Out of scope:** Batch file operations.

### PR-071 — Add session marks

- **Branch:** `feature/pr-070-session-marks`
- **Depends on:** PR-070
- **Requirements:** FR-LS-006–FR-LS-008
- **Outcome:** Add Keep, Maybe, Reject, and Favorite session-only marks.
- **Tests first:** Mark/unmark, multi-selection, filters, and folder change.
- **Acceptance:** No manager database record is created.
- **Out of scope:** Persistent manager favorites.

### PR-072 — Add recursive enumeration

- **Branch:** `feature/pr-071-recursive-view`
- **Depends on:** PR-040, PR-041
- **Requirements:** FR-BR-012
- **Outcome:** Add cancellable recursive enumeration with cycle protection.
- **Tests first:** Deep tree, symlink cycle, permission boundary, cancellation,
  and network interruption.
- **Acceptance:** Recursive mode streams results and does not block navigation.
- **Out of scope:** Recursive filesystem watching.

### PR-073 — Add file watcher

- **Branch:** `feature/pr-072-file-watcher`
- **Depends on:** PR-040, PR-063
- **Requirements:** FR-BR-008, FR-FM-010
- **Outcome:** Observe relevant external changes and invalidate affected cache.
- **Tests first:** Create, modify, rename, delete, burst coalescing, and watcher
  failure.
- **Acceptance:** Current selection is retained when its stable target remains.
- **Out of scope:** Recursive network-volume guarantees.

### PR-074 — Add recent folders

- **Branch:** `feature/pr-073-recent-folders`
- **Depends on:** PR-042, PR-062
- **Requirements:** FR-BR-011
- **Outcome:** Persist and reopen bounded recent-folder history.
- **Tests first:** Add, reorder, limit, missing folder, privacy redaction, and
  clear history.
- **Acceptance:** Recent history remains technical cache data.
- **Out of scope:** Favorites.

### PR-075 — Add rename service and UI

- **Branch:** `feature/pr-074-rename-file`
- **Depends on:** PR-070, PR-073
- **Requirements:** FR-FM-004, FR-FM-009, FR-FM-010
- **Outcome:** Rename one selected file safely and reconcile playback/cache.
- **Tests first:** Success, collision, invalid name, playing file, permission
  failure, and external race.
- **Acceptance:** The visible item and cache identity update consistently.
- **Out of scope:** Batch rename.

### PR-076 — Add move service and UI

- **Branch:** `feature/pr-075-move-files`
- **Depends on:** PR-070, PR-073
- **Requirements:** FR-FM-004, FR-FM-005
- **Outcome:** Move selected files with progress and partial-failure reporting.
- **Tests first:** Success, collision, cross-volume, cancellation, permission,
  and partial failure.
- **Acceptance:** Successful and failed targets are reported separately.
- **Out of scope:** Copy.

### PR-077 — Add copy service and UI

- **Branch:** `feature/pr-076-copy-files`
- **Depends on:** PR-076
- **Requirements:** FR-FM-004, FR-FM-005
- **Outcome:** Copy selected files with progress and collision policy.
- **Tests first:** Success, collision choices, cancellation, and partial failure.
- **Acceptance:** Originals remain unchanged.
- **Out of scope:** Manager import.

### PR-078 — Add reveal and open-with actions

- **Branch:** `feature/pr-077-external-actions`
- **Depends on:** PR-044
- **Requirements:** FR-FM-006, FR-FM-007
- **Outcome:** Reveal or open the selected file through platform adapters.
- **Tests first:** Adapter command, missing file, unsupported platform, and
  failure mapping.
- **Acceptance:** React receives no general process-launch capability.
- **Out of scope:** Drag-out.

### PR-079 — Add drag-out

- **Branch:** `feature/pr-078-file-drag-out`
- **Depends on:** PR-044
- **Requirements:** FR-FM-011
- **Outcome:** Drag selected files to compatible external applications.
- **Tests first:** Drag payload, multi-selection, missing target, and cancellation.
- **Acceptance:** Manual verification succeeds with Finder and one DAW/editor.
- **Out of scope:** DAW bridge plugin.

### PR-080 — Add configurable shortcuts

- **Branch:** `feature/pr-079-configurable-shortcuts`
- **Depends on:** PR-052, PR-062
- **Requirements:** FR-KB-003, FR-KB-004
- **Outcome:** Store, edit, validate, reset, and apply shortcut mappings.
- **Tests first:** Conflict, reserved keys, platform modifiers, persistence, and
  reset.
- **Acceptance:** Invalid conflicts cannot be saved silently.
- **Out of scope:** Multiple shortcut profiles.

## 13. Milestone J — Real-time visualizations

### PR-081 — Define visualization frame contract

- **Branch:** `feature/pr-080-visualization-contract`
- **Depends on:** PR-024
- **Requirements:** FR-VS-008, FR-VS-009
- **Outcome:** Define bounded, read-only analysis frames and drop policy.
- **Tests first:** Capacity, frame dropping, subscriber removal, and shutdown.
- **Acceptance:** Publishing never blocks the audio callback.
- **Out of scope:** FFT and rendering.

### PR-082 — Add FFT worker

- **Branch:** `feature/pr-081-fft-worker`
- **Depends on:** PR-081
- **Requirements:** FR-VS-005–FR-VS-007
- **Outcome:** Compute windowed FFT frames outside the audio callback.
- **Tests first:** Known tones, silence, mixed tones, cancellation, and lag.
- **Acceptance:** Frequency bins match fixture tolerances.
- **Out of scope:** UI.

### PR-083 — Add logarithmic analyzer

- **Branch:** `feature/pr-082-log-analyzer`
- **Depends on:** PR-082, PR-056
- **Requirements:** FR-VS-005
- **Outcome:** Render a theme-aware logarithmic frequency analyzer.
- **Tests first:** Frequency mapping, resize, disabled state, and screenshot.
- **Acceptance:** Late frames are dropped without affecting playback.
- **Out of scope:** Linear analyzer.

### PR-084 — Add linear analyzer

- **Branch:** `feature/pr-083-linear-analyzer`
- **Depends on:** PR-082, PR-056
- **Requirements:** FR-VS-006
- **Outcome:** Render a theme-aware linear frequency analyzer.
- **Tests first:** Bin mapping, resize, disabled state, and screenshot.
- **Acceptance:** Analyzer can be switched without restarting playback.
- **Out of scope:** Musical spectrum.

### PR-085 — Add musical spectrum

- **Branch:** `feature/pr-084-musical-spectrum`
- **Depends on:** PR-082, PR-056
- **Requirements:** FR-VS-007
- **Outcome:** Group frequency energy into pitch-oriented musical bands.
- **Tests first:** Known-note fixtures, tuning reference, and band boundaries.
- **Acceptance:** Known sine tones appear in expected musical bands.
- **Out of scope:** Key detection.

### PR-086 — Add visualization settings

- **Branch:** `feature/pr-085-visualization-settings`
- **Depends on:** PR-083, PR-084, PR-085, PR-062
- **Outcome:** Select visualization, quality, enabled state, and persistence.
- **Tests first:** Selection, disable, persistence, fallback, and reduced-motion.
- **Acceptance:** Disabling visualizations stops their worker load.
- **Out of scope:** Third-party visualizer plugins.

## 14. Milestone K — A–B repeat and advanced playback

### PR-087 — Define loop region

- **Branch:** `feature/pr-086-loop-region`
- **Depends on:** PR-011
- **Requirements:** FR-AU-009
- **Outcome:** Add validated A/B positions and loop-region state.
- **Tests first:** Ordering, equal points, bounds, clear, and duration change.
- **Acceptance:** Invalid regions cannot reach the audio engine.
- **Out of scope:** Playback implementation.

### PR-088 — Play A–B repeat

- **Branch:** `feature/pr-087-ab-repeat`
- **Depends on:** PR-032, PR-087
- **Requirements:** FR-AU-009
- **Outcome:** Loop the selected region.
- **Tests first:** Boundary, seek into/out of region, clear, and short region.
- **Acceptance:** Region repeats without advancing to another file.
- **Out of scope:** Waveform selection.

### PR-089 — Select A–B region on waveform

- **Branch:** `feature/pr-088-waveform-ab-selection`
- **Depends on:** PR-065, PR-088
- **Requirements:** FR-AU-009
- **Outcome:** Create, adjust, display, and clear A/B points on the waveform.
- **Tests first:** Pointer, keyboard, bounds, reversed drag, and clear.
- **Acceptance:** Displayed points reflect confirmed Rust state.
- **Out of scope:** Saving regions to managers.

### PR-090 — Add sequential playback

- **Branch:** `feature/pr-089-sequential-playback`
- **Depends on:** PR-013, PR-030
- **Requirements:** FR-AU-008
- **Outcome:** Start the next visible playable file at completion.
- **Tests first:** End, removed next item, filtered list, last item, and stop.
- **Acceptance:** Sequential playback follows the current browser ordering.
- **Out of scope:** Random playback.

### PR-091 — Add random playback

- **Branch:** `feature/pr-090-random-playback`
- **Depends on:** PR-013, PR-030
- **Requirements:** FR-AU-008
- **Outcome:** Select a random visible playable file without immediate repeat
  when alternatives exist.
- **Tests first:** Deterministic seeded selection, one item, removed items, and
  repeat avoidance.
- **Acceptance:** Random behavior is testable through injected randomness.
- **Out of scope:** Weighted or smart shuffle.

## 15. Milestone L — Sample Manager MVP

### PR-092 — Create Sample Manager database

- **Branch:** `feature/pr-091-sample-database`
- **Depends on:** PR-062
- **Requirements:** FR-SM-001
- **Outcome:** Add independent `samples.sqlite`, migrations, worker, and health
  status.
- **Tests first:** Fresh/repeat migration, rollback, backup, corruption, and
  independent failure.
- **Acceptance:** Audio Player starts when this database fails.
- **Out of scope:** Import and sample fields beyond identity.

### PR-093 — Define sample item model

- **Branch:** `feature/pr-092-sample-model`
- **Depends on:** PR-002
- **Requirements:** FR-SM-005, FR-SM-006
- **Outcome:** Define `SampleId`, location, category, tag, rating, favorite, and
  notes domain types.
- **Tests first:** Validation and identity tests.
- **Acceptance:** Model has no SQLite or UI dependency.
- **Out of scope:** Repository and analysis.

### PR-094 — Persist referenced sample

- **Branch:** `feature/pr-093-reference-sample`
- **Depends on:** PR-092, PR-093
- **Requirements:** FR-SM-002, FR-SM-003
- **Outcome:** Explicitly import one existing file by reference.
- **Tests first:** Success, duplicate policy, missing file, unsupported file,
  rollback, and no source mutation.
- **Acceptance:** Import creates one sample record and leaves the file unchanged.
- **Out of scope:** Copy and move.

### PR-095 — Copy sample into managed storage

- **Branch:** `feature/pr-094-copy-sample`
- **Depends on:** PR-094
- **Requirements:** FR-SM-003, FR-SM-004
- **Outcome:** Copy one sample into configured managed storage transactionally.
- **Tests first:** Success, collision, copy failure, database failure, and cleanup.
- **Acceptance:** Original remains and partial managed copies are cleaned safely.
- **Out of scope:** Move.

### PR-096 — Move sample into managed storage

- **Branch:** `feature/pr-095-move-sample`
- **Depends on:** PR-095
- **Requirements:** FR-SM-003, FR-SM-004
- **Outcome:** Move one sample after explicit confirmation.
- **Tests first:** Success, database failure compensation, cross-volume move,
  and cancellation.
- **Acceptance:** UI copy clearly states that the source location changes.
- **Out of scope:** Batch import.

### PR-097 — Add Sample Manager import UI

- **Branch:** `feature/pr-096-sample-import-ui`
- **Depends on:** PR-070, PR-094, PR-095, PR-096
- **Requirements:** FR-SM-002–FR-SM-004
- **Outcome:** Import selected browsed files by reference, copy, or move.
- **Tests first:** Mode choice, consequences, progress, duplicates, and partial
  failure.
- **Acceptance:** Browsing alone never invokes an import command.
- **Out of scope:** Sample library view.

### PR-098 — Add Sample Manager list

- **Branch:** `feature/pr-097-sample-list`
- **Depends on:** PR-093, PR-094
- **Requirements:** FR-SM-005
- **Outcome:** Display virtualized sample items and play a selected item.
- **Tests first:** Loading, empty, pagination/streaming, selection, and missing
  location.
- **Acceptance:** Sample playback reuses the shared playback service.
- **Out of scope:** Search, tags, and editing.

### PR-099 — Add sample tags

- **Branch:** `feature/pr-098-sample-tags`
- **Depends on:** PR-098
- **Requirements:** FR-SM-005, FR-SM-006
- **Outcome:** Create, assign, remove, and filter by sample tags.
- **Tests first:** Duplicate tag, multi-assign, remove, rollback, and filter.
- **Acceptance:** Tag writes are transactional.
- **Out of scope:** Rating and favorites.

### PR-100 — Add sample rating and favorite

- **Branch:** `feature/pr-099-sample-rating`
- **Depends on:** PR-098
- **Requirements:** FR-SM-005
- **Outcome:** Persist rating and favorite state.
- **Tests first:** Bounds, clear, toggle, batch update, and rollback.
- **Acceptance:** Browser session marks remain separate from manager favorites.
- **Out of scope:** Notes.

### PR-101 — Add sample notes and category

- **Branch:** `feature/pr-100-sample-details`
- **Depends on:** PR-098
- **Requirements:** FR-SM-005, FR-SM-006
- **Outcome:** Edit notes, instrument/type category, source, and license.
- **Tests first:** Validation, save, clear, Unicode, and rollback.
- **Acceptance:** Metadata may remain PulseSeek-only.
- **Out of scope:** Embedded tag writes.

### PR-102 — Add Sample Manager search

- **Branch:** `feature/pr-101-sample-search`
- **Depends on:** PR-099, PR-100, PR-101
- **Requirements:** FR-SM-005
- **Outcome:** Search and combine sample metadata filters.
- **Tests first:** Text, tag, category, favorite, rating, empty, and combined
  filters.
- **Acceptance:** Search remains responsive at the agreed fixture size.
- **Out of scope:** Similarity search.

### PR-103 — Detect missing sample files

- **Branch:** `feature/pr-102-missing-samples`
- **Depends on:** PR-098
- **Outcome:** Mark unavailable referenced locations without removing items.
- **Tests first:** Missing, restored, disconnected volume, and managed copy.
- **Acceptance:** Missing items remain searchable and clearly unavailable.
- **Out of scope:** Relinking.

### PR-104 — Relink missing sample

- **Branch:** `feature/pr-103-relink-sample`
- **Depends on:** PR-103
- **Outcome:** Select and validate a replacement path.
- **Tests first:** Matching file, incompatible file, duplicate path, cancellation,
  and rollback.
- **Acceptance:** Relink updates location without changing `SampleId`.
- **Out of scope:** Batch automatic relink.

## 16. Milestone M — Analysis job foundation

### PR-105 — Define cancellable analysis jobs

- **Branch:** `feature/pr-104-analysis-jobs`
- **Depends on:** PR-008
- **Requirements:** FR-SM-008
- **Outcome:** Define queued, running, completed, failed, and cancelled jobs.
- **Tests first:** Transitions, cancellation race, retry, and shutdown.
- **Acceptance:** Jobs run outside UI and audio threads.
- **Out of scope:** Any analyzer.

### PR-106 — Persist analysis versions

- **Branch:** `feature/pr-105-analysis-versions`
- **Depends on:** PR-092, PR-105
- **Requirements:** FR-SM-007, FR-SM-008
- **Outcome:** Store analyzer ID, version, status, and result ownership.
- **Tests first:** New result, stale version, retry, cancellation, and rollback.
- **Acceptance:** Algorithm upgrades can mark old results stale.
- **Out of scope:** Waveform, BPM, and key algorithms.

### PR-107 — Add loudness analysis

- **Branch:** `feature/pr-106-loudness-analysis`
- **Depends on:** PR-105, PR-106
- **Requirements:** FR-SM-007
- **Outcome:** Analyze and persist loudness and peak values.
- **Tests first:** Calibrated fixtures, silence, cancellation, and corrupt file.
- **Acceptance:** Results include analyzer version and units.
- **Out of scope:** True Peak if unsupported by the first implementation.

### PR-108 — Add BPM analysis

- **Branch:** `feature/pr-107-bpm-analysis`
- **Depends on:** PR-105, PR-106
- **Requirements:** FR-SM-007
- **Outcome:** Analyze and persist BPM with confidence.
- **Tests first:** Click-track fixtures, half/double-time cases, silence, and
  cancellation.
- **Acceptance:** Low-confidence results are distinguishable from confirmed BPM.
- **Out of scope:** Manual BPM correction UI.

### PR-109 — Add key analysis

- **Branch:** `feature/pr-108-key-analysis`
- **Depends on:** PR-105, PR-106
- **Requirements:** FR-SM-007
- **Outcome:** Analyze and persist musical key with confidence.
- **Tests first:** Key fixtures, ambiguous audio, silence, and cancellation.
- **Acceptance:** Unknown/ambiguous results are supported.
- **Out of scope:** Harmonic mixing display.

### PR-110 — Add transient analysis

- **Branch:** `feature/pr-109-transient-analysis`
- **Depends on:** PR-105, PR-106
- **Requirements:** FR-SM-007
- **Outcome:** Detect and persist transient positions.
- **Tests first:** Percussive fixtures, smooth audio, threshold, and cancellation.
- **Acceptance:** Positions are valid within file duration.
- **Out of scope:** Slice export.

### PR-111 — Add loop classification

- **Branch:** `feature/pr-110-loop-classification`
- **Depends on:** PR-105, PR-106
- **Requirements:** FR-SM-006, FR-SM-007
- **Outcome:** Estimate loop versus one-shot with confidence.
- **Tests first:** Known loops, one-shots, ambiguous clips, and cancellation.
- **Acceptance:** Classification never overwrites an explicit user choice.
- **Out of scope:** Automatic loop-point correction.

## 17. Milestone N — Music Manager MVP

### PR-112 — Create Music Manager database

- **Branch:** `feature/pr-111-music-database`
- **Depends on:** PR-062
- **Requirements:** FR-MM-001
- **Outcome:** Add independent `music.sqlite`, migrations, worker, and health
  status.
- **Tests first:** Fresh/repeat migration, rollback, backup, corruption, and
  independent failure.
- **Acceptance:** Other modules start when this database fails.
- **Out of scope:** Track import.

### PR-113 — Define music track model

- **Branch:** `feature/pr-112-music-model`
- **Depends on:** PR-002
- **Requirements:** FR-MM-004, FR-MM-005
- **Outcome:** Define `MusicTrackId`, location, title, artist, album, genre,
  rating, favorite, color, and notes types.
- **Tests first:** Validation and identity tests.
- **Acceptance:** Model has no SQLite or UI dependency.
- **Out of scope:** Cue points, loops, and analysis.

### PR-114 — Reference music track

- **Branch:** `feature/pr-113-reference-track`
- **Depends on:** PR-112, PR-113
- **Requirements:** FR-MM-002, FR-MM-003
- **Outcome:** Explicitly import one existing music file by reference.
- **Tests first:** Success, duplicate policy, missing/unsupported file, rollback,
  and no mutation.
- **Acceptance:** Import leaves the source unchanged.
- **Out of scope:** Copy and move.

### PR-115 — Copy music into managed storage

- **Branch:** `feature/pr-114-copy-track`
- **Depends on:** PR-114
- **Requirements:** FR-MM-003
- **Outcome:** Copy one track transactionally.
- **Tests first:** Success, collision, copy failure, database failure, and cleanup.
- **Acceptance:** Original remains unchanged.
- **Out of scope:** Move.

### PR-116 — Move music into managed storage

- **Branch:** `feature/pr-115-move-track`
- **Depends on:** PR-115
- **Requirements:** FR-MM-003
- **Outcome:** Move one track after explicit confirmation.
- **Tests first:** Success, compensation, cross-volume, and cancellation.
- **Acceptance:** Source-location change is explicit.
- **Out of scope:** Batch import.

### PR-117 — Add Music Manager import UI

- **Branch:** `feature/pr-116-music-import-ui`
- **Depends on:** PR-070, PR-114, PR-115, PR-116
- **Requirements:** FR-MM-002, FR-MM-003
- **Outcome:** Import selected browsed files by reference, copy, or move.
- **Tests first:** Mode, consequences, progress, duplicates, and partial failure.
- **Acceptance:** Browsing alone never imports.
- **Out of scope:** Music library view.

### PR-118 — Add Music Manager list

- **Branch:** `feature/pr-117-music-list`
- **Depends on:** PR-113, PR-114
- **Requirements:** FR-MM-004
- **Outcome:** Display virtualized tracks and play a selected track.
- **Tests first:** Loading, empty, streaming, selection, and missing location.
- **Acceptance:** Playback reuses the shared service.
- **Out of scope:** Metadata editing and filters.

### PR-119 — Edit core music metadata

- **Branch:** `feature/pr-118-music-metadata`
- **Depends on:** PR-118
- **Requirements:** FR-MM-004, FR-MM-005
- **Outcome:** Edit title, artist, album, genre, year, color, and notes.
- **Tests first:** Save, clear, validation, Unicode, and rollback.
- **Acceptance:** Changes remain PulseSeek-only unless explicitly exported.
- **Out of scope:** Writing embedded tags.

### PR-120 — Add music tags, rating, and favorite

- **Branch:** `feature/pr-119-music-tags-rating`
- **Depends on:** PR-118
- **Requirements:** FR-MM-004, FR-MM-005
- **Outcome:** Assign tags, rating, and favorite state.
- **Tests first:** Tag lifecycle, bounds, batch update, and rollback.
- **Acceptance:** Updates are transactional.
- **Out of scope:** Search.

### PR-121 — Add Music Manager search

- **Branch:** `feature/pr-120-music-search`
- **Depends on:** PR-119, PR-120
- **Requirements:** FR-MM-004
- **Outcome:** Search and combine core track filters.
- **Tests first:** Text, artist, album, genre, tag, favorite, rating, and combined
  filters.
- **Acceptance:** Search remains responsive at the agreed fixture size.
- **Out of scope:** Similarity and harmonic search.

### PR-122 — Detect and relink missing tracks

- **Branch:** `feature/pr-121-relink-track`
- **Depends on:** PR-118
- **Requirements:** FR-MM-008
- **Outcome:** Detect missing locations and relink one track.
- **Tests first:** Missing, restored, compatible replacement, cancellation, and
  rollback.
- **Acceptance:** Relink preserves `MusicTrackId`.
- **Out of scope:** Automatic batch relink.

## 18. Milestone O — Playlist Manager

### PR-123 — Create Playlist Manager database

- **Branch:** `feature/pr-122-playlist-database`
- **Depends on:** PR-062
- **Requirements:** FR-PM-001
- **Outcome:** Add independent `playlists.sqlite`, migrations, worker, and
  health status.
- **Tests first:** Fresh/repeat migration, rollback, corruption, and independent
  failure.
- **Acceptance:** Other modules start when this database fails.
- **Out of scope:** Playlist behavior.

### PR-124 — Define playlist model

- **Branch:** `feature/pr-123-playlist-model`
- **Depends on:** PR-093, PR-113
- **Requirements:** FR-PM-002, FR-PM-005
- **Outcome:** Define `PlaylistId` and typed sample/music entry references.
- **Tests first:** Ordering, typed reference, identity, and deletion semantics.
- **Acceptance:** Entry removal cannot imply source deletion.
- **Out of scope:** Persistence.

### PR-125 — Create and rename playlist

- **Branch:** `feature/pr-124-create-playlist`
- **Depends on:** PR-123, PR-124
- **Requirements:** FR-PM-003
- **Outcome:** Create, list, rename, and annotate playlists.
- **Tests first:** Validation, duplicate name policy, Unicode, save, and rollback.
- **Acceptance:** Playlist is usable without source entries.
- **Out of scope:** Duplicate and delete.

### PR-126 — Add sample to playlist

- **Branch:** `feature/pr-125-add-sample-to-playlist`
- **Depends on:** PR-094, PR-125
- **Requirements:** FR-SM-009, FR-PM-004
- **Outcome:** Append a typed `SampleId` entry.
- **Tests first:** Success, missing sample, duplicate policy, ordering, and
  rollback.
- **Acceptance:** No audio file or sample metadata is duplicated.
- **Out of scope:** Music entries.

### PR-127 — Add music to playlist

- **Branch:** `feature/pr-126-add-music-to-playlist`
- **Depends on:** PR-114, PR-125
- **Requirements:** FR-MM-007, FR-PM-004
- **Outcome:** Append a typed `MusicTrackId` entry.
- **Tests first:** Success, missing track, duplicate policy, ordering, and
  rollback.
- **Acceptance:** Mixed playlists retain typed references.
- **Out of scope:** Browser-only files.

### PR-128 — Reorder playlist entries

- **Branch:** `feature/pr-127-reorder-playlist`
- **Depends on:** PR-126, PR-127
- **Requirements:** FR-PM-003
- **Outcome:** Move one or multiple entries deterministically.
- **Tests first:** First/last, range, same position, mixed types, and rollback.
- **Acceptance:** Ordering is stable and gap-free.
- **Out of scope:** Drag UI.

### PR-129 — Add Playlist Manager UI

- **Branch:** `feature/pr-128-playlist-ui`
- **Depends on:** PR-125, PR-128
- **Requirements:** FR-PM-003, FR-PM-004
- **Outcome:** Create/select playlists, add manager items, and reorder by drag or
  keyboard.
- **Tests first:** Create, add, reorder, keyboard, missing reference, and error.
- **Acceptance:** Mixed sample/music entries are visibly distinguishable.
- **Out of scope:** Browser-only entry import and export.

### PR-130 — Duplicate playlist

- **Branch:** `feature/pr-129-duplicate-playlist`
- **Depends on:** PR-128
- **Requirements:** FR-PM-003
- **Outcome:** Duplicate playlist metadata and ordered references.
- **Tests first:** Empty, mixed entries, naming conflict, and rollback.
- **Acceptance:** Source and duplicate have independent playlist identities.
- **Out of scope:** Duplicating source manager items.

### PR-131 — Delete playlist safely

- **Branch:** `feature/pr-130-delete-playlist`
- **Depends on:** PR-128
- **Requirements:** FR-PM-003, FR-PM-005
- **Outcome:** Delete a playlist after confirmation.
- **Tests first:** Empty, populated, cancel, rollback, and source survival.
- **Acceptance:** Samples, tracks, and audio files are unchanged.
- **Out of scope:** Undo.

### PR-132 — Add browser selection to playlist

- **Branch:** `feature/pr-131-browser-to-playlist`
- **Depends on:** PR-070, PR-097, PR-117, PR-129
- **Requirements:** FR-PM-004
- **Outcome:** Explicitly choose Sample or Music import, then add resulting item
  to a playlist.
- **Tests first:** Manager choice, import mode, cancellation, partial failure,
  and ordering.
- **Acceptance:** A browsed file never becomes an untyped playlist reference.
- **Out of scope:** Temporary path-only playlist entries.

## 19. Milestone P — Playlist exports

### PR-133 — Define playlist export port

- **Branch:** `feature/pr-132-export-port`
- **Depends on:** PR-124
- **Requirements:** FR-PM-006
- **Outcome:** Define resolved export entries, validation, warnings, and output
  result.
- **Tests first:** Missing reference, unavailable file, mixed types, and order.
- **Acceptance:** Export adapters do not query manager databases directly.
- **Out of scope:** File formats.

### PR-134 — Export M3U8

- **Branch:** `feature/pr-133-export-m3u8`
- **Depends on:** PR-133
- **Requirements:** FR-PM-006
- **Outcome:** Export Unicode-safe ordered M3U8.
- **Tests first:** Relative/absolute paths, Unicode, missing file warning, and
  deterministic fixture.
- **Acceptance:** Fixture opens in two external players.
- **Out of scope:** Legacy M3U.

### PR-135 — Export M3U

- **Branch:** `feature/pr-134-export-m3u`
- **Depends on:** PR-134
- **Requirements:** FR-PM-006
- **Outcome:** Export legacy-compatible M3U with explicit encoding policy.
- **Tests first:** Encoding, unsupported characters, order, and warnings.
- **Acceptance:** Encoding loss is reported, never silent.
- **Out of scope:** CSV.

### PR-136 — Export JSON

- **Branch:** `feature/pr-135-export-json`
- **Depends on:** PR-133
- **Requirements:** FR-PM-006
- **Outcome:** Export versioned JSON with typed entries and warnings.
- **Tests first:** Schema version, sample/music entries, Unicode, and
  deterministic fixture.
- **Acceptance:** JSON is round-trip parseable by the same schema.
- **Out of scope:** Import.

### PR-137 — Export CSV

- **Branch:** `feature/pr-136-export-csv`
- **Depends on:** PR-133
- **Requirements:** FR-PM-006
- **Outcome:** Export documented UTF-8 CSV columns.
- **Tests first:** Quoting, delimiters, newlines, Unicode, mixed entries, and
  deterministic fixture.
- **Acceptance:** CSV imports correctly into a reference spreadsheet tool.
- **Out of scope:** DJ-specific columns.

### PR-138 — Add export UI and dry-run summary

- **Branch:** `feature/pr-137-export-ui`
- **Depends on:** PR-134, PR-135, PR-136, PR-137
- **Requirements:** FR-PM-006
- **Outcome:** Choose format/path, preview warnings, export, and reveal result.
- **Tests first:** Format choice, warnings, cancellation, success, and failure.
- **Acceptance:** Missing items are reported before writing.
- **Out of scope:** DJ sync.

## 20. Milestone Q — DJ adapters

Every DJ integration begins only after its current documented exchange format
has been captured in a versioned ADR. Research is performed within the PR and
must end in production documentation, fixtures, and tests; there are no
research-only branches.

### PR-139 — Document Rekordbox exchange contract

- **Branch:** `feature/pr-138-rekordbox-contract`
- **Depends on:** PR-133
- **Requirements:** FR-PM-008
- **Outcome:** Add ADR, supported XML subset, fixtures, and adapter port mapping.
- **Tests first:** Parse official-format fixtures into neutral export entries.
- **Acceptance:** Supported and unsupported fields are explicit.
- **Out of scope:** Writing XML.

### PR-140 — Export Rekordbox XML

- **Branch:** `feature/pr-139-rekordbox-export`
- **Depends on:** PR-139
- **Requirements:** FR-PM-007, FR-PM-008
- **Outcome:** Generate deterministic XML for the supported subset.
- **Tests first:** Escaping, paths, metadata, playlist order, and round-trip
  fixture.
- **Acceptance:** Rekordbox accepts a manually verified export fixture.
- **Out of scope:** Modifying Rekordbox databases.

### PR-141 — Add Rekordbox export UI

- **Branch:** `feature/pr-140-rekordbox-ui`
- **Depends on:** PR-138, PR-140
- **Outcome:** Add target selection, dry run, warnings, export, and audit record.
- **Tests first:** Dry run, confirmation, success, failure, and audit entry.
- **Acceptance:** No external data is changed before confirmation.
- **Out of scope:** Two-way synchronization.

### PR-142 — Document Serato safe exchange contract

- **Branch:** `feature/pr-141-serato-contract`
- **Depends on:** PR-133
- **Requirements:** FR-PM-009
- **Outcome:** Add ADR, safe supported mechanism, fixtures, and limitations.
- **Tests first:** Parse supported fixture or validate supported output contract.
- **Acceptance:** Undocumented internal database writes are explicitly excluded.
- **Out of scope:** Export implementation.

### PR-143 — Implement Serato safe export

- **Branch:** `feature/pr-142-serato-export`
- **Depends on:** PR-142
- **Requirements:** FR-PM-007, FR-PM-009
- **Outcome:** Implement only the documented/safe exchange mechanism.
- **Tests first:** Deterministic fixtures, path handling, warnings, and
  unsupported fields.
- **Acceptance:** Manual compatibility test succeeds without internal DB writes.
- **Out of scope:** Unsupported Serato metadata.

### PR-144 — Document Engine DJ exchange contract

- **Branch:** `feature/pr-143-engine-dj-contract`
- **Depends on:** PR-133
- **Requirements:** FR-PM-010
- **Outcome:** Add ADR, supported mechanism, fixtures, and limitations.
- **Tests first:** Contract fixture tests.
- **Acceptance:** Safe read/write boundaries are explicit.
- **Out of scope:** Adapter implementation.

### PR-145 — Implement Engine DJ adapter

- **Branch:** `feature/pr-144-engine-dj-adapter`
- **Depends on:** PR-144
- **Requirements:** FR-PM-007, FR-PM-010
- **Outcome:** Implement the documented safe subset with dry run and audit log.
- **Tests first:** Deterministic fixtures, warnings, backup, and failure recovery.
- **Acceptance:** Manual compatibility test succeeds.
- **Out of scope:** Undocumented database fields.

## 21. Milestone R — Effect chain and visualizer plugins

### PR-146 — Define effect-chain contract

- **Branch:** `feature/pr-145-effect-chain`
- **Depends on:** PR-024
- **Requirements:** FR-PL-004, FR-PL-005
- **Outcome:** Define ordered processors, bypass, state, and real-time contract.
- **Tests first:** Ordering, bypass, failure, and lifecycle.
- **Acceptance:** Contract prohibits blocking and allocation in process calls.
- **Out of scope:** Third-party plugins.

### PR-147 — Add built-in gain effect

- **Branch:** `feature/pr-146-gain-effect`
- **Depends on:** PR-146
- **Outcome:** Prove the chain with a tested built-in gain processor.
- **Tests first:** Gain, bypass, ordering, and state restore.
- **Acceptance:** Global bypass restores the clean path.
- **Out of scope:** VST3.

### PR-148 — Define visualizer plugin API

- **Branch:** `feature/pr-147-visualizer-api`
- **Depends on:** PR-081
- **Requirements:** FR-PL-008
- **Outcome:** Define a versioned read-only visualization-frame API and manifest.
- **Tests first:** Version compatibility, malformed manifest, frame access, and
  unload.
- **Acceptance:** Plugins cannot control playback.
- **Out of scope:** Dynamic loading.

### PR-149 — Load one PulseSeek visualizer plugin

- **Branch:** `feature/pr-148-load-visualizer`
- **Depends on:** PR-148
- **Requirements:** FR-VS-008, FR-PL-008
- **Outcome:** Discover, validate, load, render, and unload one plugin.
- **Tests first:** Valid, incompatible, corrupt, duplicate, and unload fixtures.
- **Acceptance:** A failing plugin leaves built-in visualizations usable.
- **Out of scope:** Marketplace and hot reload.

### PR-150 — Add plugin safe mode

- **Branch:** `feature/pr-149-plugin-safe-mode`
- **Depends on:** PR-149
- **Requirements:** FR-PL-006, FR-PL-007
- **Outcome:** Start with third-party plugins disabled and reset scan state.
- **Tests first:** Crash marker, safe startup, user enable, and persistent choice.
- **Acceptance:** Plugin failure cannot prevent Audio Player startup.
- **Out of scope:** Out-of-process VST scan.

### PR-151 — Document VST3 hosting constraints

- **Branch:** `feature/pr-150-vst3-host-contract`
- **Depends on:** PR-146
- **Requirements:** FR-PL-001–FR-PL-003
- **Outcome:** Add ADR for SDK/license, lifecycle, scanning, state, and platform
  constraints plus a compile-time host interface.
- **Tests first:** Host interface contract with a fake plugin.
- **Acceptance:** The PR produces versioned production contracts and tested
  maintainable code.
- **Out of scope:** Loading a real VST3.

### PR-152 — Scan VST3 plugins out of process

- **Branch:** `feature/pr-151-vst3-scanner`
- **Depends on:** PR-151
- **Requirements:** FR-PL-001, FR-PL-007
- **Outcome:** Scan configured plugin paths in an isolated helper process.
- **Tests first:** Valid fake, crash, timeout, duplicate, and cache invalidation.
- **Acceptance:** Scanner crash does not crash the desktop app.
- **Out of scope:** Audio processing.

### PR-153 — Load one VST3 effect

- **Branch:** `feature/pr-152-load-vst3-effect`
- **Depends on:** PR-147, PR-152
- **Requirements:** FR-PL-001, FR-PL-002, FR-PL-004
- **Outcome:** Instantiate one validated effect in the effect chain.
- **Tests first:** Lifecycle through host contract, bypass, failure, and state.
- **Acceptance:** Manual test processes audio and global bypass restores clean
  output.
- **Out of scope:** Plugin editor UI.

### PR-154 — Persist VST3 state

- **Branch:** `feature/pr-153-vst3-state`
- **Depends on:** PR-153, PR-062
- **Requirements:** FR-PL-009
- **Outcome:** Save and restore plugin-chain state outside audio files.
- **Tests first:** Save, restore, missing plugin, incompatible state, and reset.
- **Acceptance:** Audio files and embedded metadata remain unchanged.
- **Out of scope:** Preset browser.

## 22. Milestone S — DAW bridge

### PR-155 — Define local bridge protocol

- **Branch:** `feature/pr-154-daw-bridge-protocol`
- **Depends on:** PR-102
- **Requirements:** FR-SM-010, FR-SM-011
- **Outcome:** Define versioned local IPC for discovery, search, preview request,
  and file transfer.
- **Tests first:** Version negotiation, authentication scope, malformed request,
  and disconnect.
- **Acceptance:** Protocol exposes application services, not SQLite.
- **Out of scope:** VST3 binary.

### PR-156 — Add desktop bridge server

- **Branch:** `feature/pr-155-desktop-bridge`
- **Depends on:** PR-155
- **Requirements:** FR-SM-011
- **Outcome:** Serve the minimal protocol locally with explicit enable/disable.
- **Tests first:** Start/stop, local-only binding, client lifecycle, invalid
  request, and shutdown.
- **Acceptance:** Bridge is disabled safely when not configured.
- **Out of scope:** DAW plugin.

### PR-157 — Create DAW VST3 shell

- **Branch:** `feature/pr-156-daw-vst3-shell`
- **Depends on:** PR-151, PR-155
- **Requirements:** FR-SM-010
- **Outcome:** Build a loadable VST3 that negotiates protocol version.
- **Tests first:** Protocol client tests and plugin lifecycle contract.
- **Acceptance:** A reference DAW loads the plugin and reports connection state.
- **Out of scope:** Search and preview.

### PR-158 — Search samples from DAW

- **Branch:** `feature/pr-157-daw-sample-search`
- **Depends on:** PR-156, PR-157
- **Requirements:** FR-SM-010
- **Outcome:** Search and filter Sample Manager items from the plugin.
- **Tests first:** Query, pagination, disconnect, empty, and incompatible version.
- **Acceptance:** Results contain stable IDs and safe display metadata.
- **Out of scope:** Preview.

### PR-159 — Preview sample from DAW

- **Branch:** `feature/pr-158-daw-preview`
- **Depends on:** PR-158
- **Requirements:** FR-SM-010
- **Outcome:** Request start/stop preview through the desktop playback service.
- **Tests first:** Start, replace, stop, disconnect, unavailable file, and
  concurrent desktop playback policy.
- **Acceptance:** The DAW plugin does not implement a second library database.
- **Out of scope:** Tempo synchronization.

### PR-160 — Transfer sample to DAW

- **Branch:** `feature/pr-159-daw-transfer`
- **Depends on:** PR-158
- **Requirements:** FR-SM-010
- **Outcome:** Transfer or drag an authorized sample file into the DAW.
- **Tests first:** Referenced/managed path, missing file, permission, cancellation,
  and cleanup.
- **Acceptance:** Manual test succeeds in the reference DAW.
- **Out of scope:** Audio Unit, CLAP, and tempo synchronization.

## 23. Release preparation PRs

### PR-161 — Add cross-platform CI matrix

- **Branch:** `ci/pr-160-platform-ci`
- **Depends on:** PR-053
- **Outcome:** Build and test on macOS, Windows, and Linux.
- **Tests/verification:** Successful matrix run with documented platform
  exclusions.
- **Acceptance:** Failures are isolated by platform and use stable check names.
- **Out of scope:** Installers.

### PR-162 — Add macOS application bundle

- **Branch:** `build/pr-161-macos-bundle`
- **Depends on:** PR-053
- **Outcome:** Configure signed-ready Apple Silicon application bundle metadata.
- **Tests/verification:** Install and launch an unsigned development artifact.
- **Acceptance:** Bundle contains required assets and no development server.
- **Out of scope:** Signing, notarization, and Intel.

### PR-163 — Add universal macOS build

- **Branch:** `build/pr-162-universal-macos`
- **Depends on:** PR-162
- **Outcome:** Build Apple Silicon and Intel universal artifact.
- **Tests/verification:** Architecture inspection and launch on available
  reference systems.
- **Acceptance:** One artifact contains both supported architectures.
- **Out of scope:** Notarization.

### PR-164 — Add Windows installer

- **Branch:** `build/pr-163-windows-installer`
- **Depends on:** PR-161
- **Outcome:** Configure Windows installer and WebView2 strategy.
- **Tests/verification:** Clean VM installation, launch, uninstall, and size.
- **Acceptance:** Installer behavior and runtime requirements are documented.
- **Out of scope:** Store distribution.

### PR-165 — Add Linux packages

- **Branch:** `build/pr-164-linux-packages`
- **Depends on:** PR-161
- **Outcome:** Configure the selected initial Linux package formats.
- **Tests/verification:** Clean environment installation, launch, uninstall, and
  dependency report.
- **Acceptance:** Supported distributions and WebKit requirements are explicit.
- **Out of scope:** Every Linux distribution.

## 24. First delivery sequence

The first development objective is PR-001 through PR-054.

Recommended checkpoints:

| Checkpoint | PRs     | Demonstrable outcome                           |
| ---------- | ------- | ---------------------------------------------- |
| A          | 001–009 | Governed, tested, buildable desktop shell      |
| B          | 010–020 | Tested playback domain and initial decoders    |
| C          | 021–033 | Headless audio player with device recovery     |
| D          | 034–037 | Typed and restricted Tauri bridge              |
| E          | 038–045 | Folder tree and virtualized playable-file list |
| F          | 046–054 | Complete P0 user workflow and measurements     |

Do not begin the Sample Manager before checkpoint F is complete and stable.

## 25. Definition of complete plan item

A PR in this plan is complete only when:

- Its declared dependencies are already merged.
- Its test was observed failing before implementation where applicable.
- Every acceptance statement is demonstrated.
- Out-of-scope behavior was not added incidentally.
- Formatting, linting, type checks, affected tests, and builds pass.
- The PR description contains risks and manual verification.
- No specification outside its explicit requirements was silently changed.
- No code was pushed directly to `develop`.
