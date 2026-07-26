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

## PR review command

When the user invokes `/review <PR_NUMBER>`, immediately start reviewing that
GitHub pull request by first reading and then following
[`docs/PR-REVIEW.md`](docs/PR-REVIEW.md). A review request authorizes inspection
and non-destructive verification only; do not modify code, push changes,
retarget, close, or merge the pull request without the user's explicit
approval.

## Implementation command

When the user invokes:

```text
/implement <TYPE> <REQUEST>
```

interpret everything after `<TYPE>` as a free-form implementation request.
The request may reference a PR-sized item, issue, acceptance criteria, or a
local document, for example:

```text
/implement feature Implement PR-002 from @spec/implementation-plan.md
```

Supported types and their branch prefixes are:

| Type | Branch |
| --- | --- |
| `feature` | `feature/<derived-name>` |
| `bugfix` | `fix/<derived-name>` |
| `hotfix` | `hotfix/<derived-name>` |
| `refactor` | `refactor/<derived-name>` |
| `chore` | `chore/<derived-name>` |
| `docs` | `docs/<derived-name>` |
| `test` | `test/<derived-name>` |
| `ci` | `ci/<derived-name>` |
| `build` | `build/<derived-name>` |

Derive a concise lowercase kebab-case branch name from the referenced item and
request. Prefer an identifier plus a short purpose when available, such as
`feature/pr-002-audio-folder-browser`. Show the proposed branch
names before creating them.

### Preparation and planning

The command starts with preparation and planning, not immediate coding:

1. Read `AGENTS.md` and every document explicitly referenced by the request.
   Resolve references such as `@spec/implementation-plan.md` relative to the
   repository root.
2. Locate the exact requested item inside the document and read enough
   surrounding context to understand its dependencies, acceptance criteria,
   exclusions, and intended PR boundary.
3. Inspect the repository and current implementation. Do not assume the plan
   document matches the code.
4. Inspect the working tree and fetch current remote state without discarding,
   stashing, or overwriting user changes.
5. Create the branch from the latest appropriate base: `develop` for ordinary
   work, or the latest released `main` for an explicitly urgent `hotfix`.
6. If the branch already exists, inspect and reuse it only when it
   belongs to the same requested item and contains no unrelated work.
7. Propose a complete implementation plan before editing production files.

The proposed plan must identify:

- The requirement, acceptance criteria, and explicit non-goals.
- The branch and base branch.
- The affected domain, application, adapter, Tauri, and React boundaries.
- The sequence of small implementation steps.
- The failing tests to write first and the expected Red → Green progression.
- Audio-thread, filesystem, database, privacy, accessibility, and theme risks
  that apply.
- Any dependency or ADR decision requiring separate approval.
- The validation commands and manual checks to run.
- The expected commit and PR boundary.

Present the plan and wait for the user's confirmation before editing production
files. Clearly call out any ambiguity, architectural choice, structural
dependency, requested edit to `spec/`, destructive behavior, or scope
materially larger than one reviewable PR. Resolve those points explicitly
before implementation.

### Implementation and handoff

Implement only the referenced item and its necessary tests and documentation.
Follow Red → Green → Refactor for production behavior, run focused checks
during development, then run every applicable required command before
handoff.

Finish by reporting:

- The completed plan items and any justified deviation.
- Changed files and user-visible behavior.
- Red and Green test evidence.
- Full validation and manual-check results.
- Remaining risks, follow-up work, and suggested Conventional Commit message.

Invoking `/implement` authorizes reading referenced files, creating the local
branch, editing files within the requested scope, and
running non-destructive validation. It does not authorize committing, pushing,
creating or modifying a pull request, merging, editing `spec/`, adding a
structural dependency, or changing an accepted architecture decision without
the separate approvals required by this document.

## Release command

When the user invokes:

```text
/release <VERSION>
```

immediately start preparing that PulseSeek version by first reading and then
following [`docs/RELEASE.md`](docs/RELEASE.md). If the user omits the version,
inspect the changes since the latest tag, propose the appropriate Semantic
Version, and wait for approval before creating a release branch.

Invoking `/release <VERSION>` authorizes creating and switching to the matching
local `release/<VERSION>` branch, but no remote or publishing action.

When the user asks to prepare, publish, or otherwise perform a release, first
read and follow [`docs/RELEASE.md`](docs/RELEASE.md). Drafting a release does
not authorize pushing a branch, merging a pull request, creating a tag,
publishing a GitHub Release, or uploading artifacts; each public release action
requires the user's explicit approval as described in that playbook.

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
