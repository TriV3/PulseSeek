# Development Workflow

## Branches

- `main` contains releasable versions.
- `develop` is the integration branch.
- `feature/*` branches target `develop`.
- `release/*` branches target `main`.
- `hotfix/*` branches are reserved for urgent production fixes.

The repository owner has a direct-push exception on `develop`, but production
code should still use a feature branch and PR by default. The exception is for
urgent or documentary changes.

## Issue and PR size

- One issue describes one observable behavior or one bounded technical task.
- One branch normally addresses one issue.
- Prefer PRs below 400 changed lines of production logic.
- Separate refactoring from behavior changes when practical.
- Use internal feature flags for incomplete work; do not leave commented code.

## Commits

Use Conventional Commits:

```text
feat: add output device selector
fix: prevent loop boundary click
test: cover missing audio device
refactor: isolate decoder registry
docs: record audio engine decision
chore: configure Rust linting
```

Commits should explain one coherent change. Do not claim tests passed when they
were skipped.

## Pull requests

Each PR describes:

- User-visible behavior
- Relevant specification or issue
- Architecture impact
- Audio real-time risks
- Data and file safety risks
- Tests written and commands run
- Manual verification
- Screenshots or recordings for visual changes
- Performance measurements when applicable

## Continuous integration

GitHub Actions will run:

- `cargo fmt --all --check`
- Clippy with warnings denied
- Rust unit and integration tests
- TypeScript formatting and linting
- Type checking
- Vitest
- Tauri production build
- Critical Playwright flows when available
- Dependency and license checks

Required checks are added to GitHub branch protections only after the workflows
exist and have produced their stable check names.

## Dependency changes

Structural dependencies require explicit approval and a short justification:

- What capability is needed?
- Why is the standard library or an existing dependency insufficient?
- Is the project maintained?
- Is its license compatible with MPL-2.0?
- What is its binary-size and security impact?
- Can it run outside the audio callback?

Use Dependabot for proposals only. Automated updates are never auto-merged.

## Architecture decisions

Create an ADR when a choice:

- Changes a module boundary
- Changes persistence ownership
- Introduces a runtime or framework
- Changes the audio thread model
- Defines a public plugin or IPC contract
- Is expensive to reverse

Accepted ADRs are immutable. Supersede them with a new ADR rather than rewriting
history.

## Definition of done

A change is ready when:

- It was developed through TDD where required.
- Relevant tests pass.
- Formatting, linting, and type checks pass.
- Failure and cancellation behavior are implemented.
- Keyboard accessibility is covered.
- Logs and errors are actionable and privacy-aware.
- No prohibited work occurs on the audio callback.
- Documentation and ADRs are updated.
- The PR is small enough to review confidently.

## Release flow

1. Create `release/<version>` from `develop`.
2. Stabilize without adding unrelated features.
3. Run the full platform matrix.
4. Update changelog and version metadata.
5. Open a PR into `main`.
6. Tag the merge commit.
7. Merge or synchronize release changes back into `develop`.
