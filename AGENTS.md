# PulseSeek Agent Rules

These instructions apply to every agent working in this repository. More
specific `AGENTS.md` files may be added in subdirectories later, but they must
not weaken these rules.

## Product boundaries

- The Audio Player must work without any manager database.
- Browsing or playing a file must never import it.
- Sample Manager, Music Manager, and Playlist Manager are separate domains.
- Each manager owns its own SQLite database and migrations.
- Shared audio, filesystem, metadata, analysis, and UI services must not merge
  manager domain models.
- Hosting effect/visualizer plugins and providing a DAW bridge plugin are
  separate systems.

## Architecture

- Use Tauri 2 with React and strict TypeScript for the desktop UI.
- Keep business logic, audio, filesystem operations, persistence, analysis, and
  plugin infrastructure in Rust.
- Follow ports-and-adapters dependency direction.
- Domain code must not depend on Tauri, React, SQLite, `cpal`, or other concrete
  adapters.
- React must not access the filesystem, databases, or audio devices directly.
- Expose narrow, typed Tauri commands and versioned events.
- Never add a crate until it represents a real dependency boundary.
- Record expensive-to-reverse decisions in `docs/adr/`.

## Test-driven development

- Use Red → Green → Refactor for production behavior.
- Write the failing test before implementing domain behavior or a bug fix.
- Rust domain and application-service behavior requires strict TDD.
- React tests must assert user-visible behavior, not implementation details.
- Use real temporary SQLite databases for persistence tests.
- Prefer small handwritten fakes over mocking frameworks.
- Technical investigations must occur inside a bounded `feature/*` PR and end
  with versioned documentation, fixtures, tests, or maintainable production
  code.
- Do not reduce or delete a meaningful test merely to make a change pass.

## Real-time audio safety

- Never allocate, lock, log, perform I/O, execute SQL, run FFTs, or communicate
  with React from the audio callback.
- Playback has priority over waveform generation, visualization, and analysis.
- Drop late visualization frames instead of delaying audio.
- Keep decoding, analysis, filesystem, database, and plugin scanning on
  dedicated workers.
- A plugin or visualizer must never own the playback clock.

## Files and data safety

- Browsing is read-only.
- Move deletion targets to the operating system trash by default.
- Never permanently delete user audio without an explicit, separate action.
- Never silently modify source audio or embedded metadata.
- Never write undocumented third-party DJ databases by default.
- Use transactions for multi-step manager writes.
- Back up databases before destructive migrations.
- Do not use cross-database SQLite foreign keys.

## Frontend rules

- Use React, strict TypeScript, Zustand, TanStack Virtual, and TanStack Table.
- Use Tailwind CSS and Radix UI primitives behind PulseSeek components.
- Use semantic design tokens; do not hard-code theme colors in feature
  components.
- Support light, dark, system, Midnight Blue, and High Contrast themes.
- Keep high-frequency drawing outside React renders.
- Use Canvas 2D for the first waveform renderer and an abstraction that permits
  a later WebGL renderer.
- All primary workflows must be keyboard accessible.

## Rust rules

- Use the pinned stable toolchain in `rust-toolchain.toml`.
- Format with `cargo fmt`.
- Treat Clippy warnings as errors.
- Use `thiserror` for typed library/domain errors and `anyhow` only at
  executable boundaries where appropriate.
- Prefer explicit repositories and application services over generic framework
  abstractions.
- Use `tracing` for structured diagnostics and never log audio content.
- Keep `Cargo.lock` versioned for the desktop application.

## TypeScript rules

- Keep TypeScript `strict` enabled.
- Do not use `any` without a local comment explaining why a safe type is
  impossible.
- Validate data crossing the Tauri boundary.
- Keep Rust as the source of truth for playback, files, and manager state.
- Use Vitest and React Testing Library for behavior tests.

## Dependencies

- Do not add a structural dependency without explicit user approval.
- Prefer maintained dependencies with compatible licenses and small,
  auditable APIs.
- Check Rust dependencies with `cargo-deny`.
- Lock frontend dependencies with `pnpm-lock.yaml`.
- Automated dependency updates may open PRs but must never merge without tests.

## Git workflow

- Work from `develop`.
- Use `feature/<short-name>` for production changes.
- Use `release/<version>` for releases and `hotfix/<short-name>` for urgent
  production fixes.
- Target ordinary PRs at `develop`.
- Target release PRs at `main`.
- Use Conventional Commits.
- Keep refactors separate from behavior changes when practical.
- Prefer independently reviewable PRs, ideally below 400 changed lines of
  production logic.

## Agent authority

Agents may, without additional permission:

- Read the repository and local specifications.
- Create local feature branches.
- Edit code and related documentation within the requested scope.
- Add tests before implementation.
- Run formatting, linting, tests, builds, and non-destructive diagnostics.

Agents must receive explicit user authorization before:

- Pushing to GitHub.
- Creating, merging, closing, or retargeting a PR.
- Pushing directly to `develop`.
- Modifying GitHub protections or repository permissions.
- Changing an accepted architecture decision.
- Adding a structural dependency.
- Editing anything under `spec/`.
- Running a destructive database migration.
- Deleting user files or real user data.

## Definition of done

A feature is done only when:

- Its behavior and failure modes are documented.
- The relevant test failed first and now passes.
- Unit and integration tests cover meaningful behavior.
- Formatting, linting, type checks, and applicable builds pass.
- The UI is keyboard accessible.
- Work is kept off the UI and audio threads where required.
- Logs provide diagnostic context without exposing private paths by default.
- Platform-specific behavior has a documented fallback.
- The implementation satisfies a referenced requirement or acceptance criterion.

## Required commands before handoff

Run the commands that exist for the current project phase:

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
pnpm format:check
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

Do not claim a command passed if it was unavailable, skipped, or not run.
