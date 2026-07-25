# Test-Driven Development

## Policy

PulseSeek uses Red → Green → Refactor.

1. Write a test describing one observable behavior.
2. Run it and confirm that it fails for the expected reason.
3. Implement the smallest correct behavior.
4. Run the focused test.
5. Run the relevant suite.
6. Refactor while keeping tests green.

Production behavior and bug fixes require a failing test first. A test that
passes before the fix does not demonstrate the bug.

## Technical investigations

Bounded feature PRs may investigate:

- UI framework performance
- Decoder support
- Audio-device behavior
- Short-loop timing
- Plugin SDK constraints

There are no disposable investigation branches and no TDD exemption for code
that will be merged. An investigation PR must finish with versioned
documentation, fixtures, tests, or maintainable production code. Exploratory
code that does not meet production standards is removed before the PR is
merged.

## Rust testing

Tools:

- `cargo test`
- `rstest` for parameterized cases and fixtures
- `proptest` for important invariants
- Temporary directories for filesystem integration tests
- Real temporary SQLite databases for migration and repository tests
- Handwritten fakes for audio devices, decoders, clocks, and filesystem ports

Avoid a mocking framework by default. Prefer small behavior-focused fakes that
make test intent obvious.

Test:

- Domain state transitions
- Playback modes and boundary conditions
- Sorting, filtering, and selection
- Path normalization and cache keys
- Import decisions
- Database migrations and rollback
- Missing files and disconnected devices
- Cancellation and partial failure

Audio callback code must be tested for bounded work and absence of prohibited
operations.

## Frontend testing

Tools:

- Vitest
- React Testing Library
- Playwright for critical end-to-end flows

React tests assert visible behavior and accessible interaction:

- What the user sees
- What the user can select
- Which command is requested
- How loading, failure, and disabled states behave
- Keyboard navigation
- Theme and contrast behavior

Do not assert internal component state, private functions, or CSS class
implementation unless they are the public contract.

Tauri commands are represented by typed fake ports in component and feature
tests.

## End-to-end scope

Keep the E2E suite small and valuable:

- Open folder and display decodable files
- Click file and start playback
- Select output device
- Switch one-shot and loop modes
- Seek through the waveform
- Move a file to trash
- Import explicitly into a manager
- Add an item to a playlist
- Export a playlist

Hardware audio verification also uses a documented manual checklist because
real device behavior cannot be fully represented in CI.

## Coverage

PulseSeek does not use an arbitrary global percentage as a target.

Coverage is mandatory for:

- Domain rules
- Error branches
- State transitions
- Data migrations
- Destructive-operation guards
- Public module contracts

Generated bindings, trivial styling, and defensive platform glue may have lower
coverage when their behavior is covered at a more appropriate level.

## Test naming

Test names describe behavior:

```text
loop_mode_restarts_at_end_of_file
unsupported_files_are_hidden_by_default
deleting_playlist_does_not_delete_audio
device_loss_pauses_and_selects_fallback
```

Use Arrange / Act / Assert within tests when it improves readability.

## CI order

Fast feedback runs first:

1. Formatting
2. Static analysis and type checking
3. Unit tests
4. Integration tests
5. Builds
6. Critical E2E tests
7. Performance and platform-specific scheduled suites
