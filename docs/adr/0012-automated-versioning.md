# ADR 0012: Automated Semantic Versioning with release-please

- Status: Accepted
- Date: 2026-08-16

## Context

PulseSeek is heading toward its first stable release and must adopt a
versioning system that satisfies three requirements:

1. The application starts at `1.0.0` and follows Semantic Versioning.
2. The version is displayed dynamically in the startup splash and at the
   bottom of the Options menu, and matches the GitHub release tag.
3. The version must never be edited by hand: it is derived from the commit
   history and propagated automatically.

Today the version is duplicated across seven `Cargo.toml` files (the
workspace members), `package.json`, and `src-tauri/tauri.conf.json`, all
pinned to `0.1.0`, and there is no `CHANGELOG.md` and no version tag. The
release playbook in `docs/RELEASE.md` requires the operator to propose and
approve every version number manually.

## Decision

### Single source of truth: the Rust package manifest

`src-tauri/Cargo.toml` keeps the application version. The bundle version in
`src-tauri/tauri.conf.json` mirrors it and is written by release-please (an
`extra-files` JSON updater), because the official `tauri-action` reads the
bundle version from `tauri.conf.json` to name and attach release assets. A
test enforces that the two values never diverge.

The renderer reads the same value at build time:

- `src/versionSource.ts` parses `src-tauri/Cargo.toml` and injects the
  version into `index.html` through a small Vite plugin, so the startup
  splash (static HTML rendered before React mounts) shows the version.
- Vite's `define` exposes `__PULSESEEK_VERSION__`, consumed by
  `src/version.ts`, so the Options menu footer shows the version.

### Automatic versioning and changelog: release-please

[release-please](https://github.com/googleapis/release-please) (Google's
maintained GitHub Action) manages version bumps, `CHANGELOG.md`, tags, and
GitHub Releases:

- `release-please-config.json` declares the Rust workspace as package `"."`,
  enables the official `cargo-workspace` plugin (which bumps every workspace
  crate through the dependency graph and refreshes `Cargo.lock`), and syncs
  `package.json` through an `extra-files` JSON updater.
- `.release-please-manifest.json` seeds the released version at `1.0.0`, and
  `bootstrap-sha` points at the current `main` head so the entire existing
  history counts as already released.
- `.github/workflows/release.yml` runs on every push to `main`. When new
  commits are detected, release-please opens a release pull request
  (version bump + changelog); merging that pull request creates the annotated
  `vX.Y.Z` tag on the exact merge commit and drafts/publishes the GitHub
  Release with the changelog.
- `changelog-sections` maps Conventional Commit types onto the
  `Added` / `Changed` / `Fixed` / `Removed` categories used by
  `docs/RELEASE.md`.

The quality workflow now also runs on pull requests targeting `main`, so
release pull requests receive the same checks as feature branches.

### Commit message enforcement

Because the derived version and changelog are only as good as the commit
messages, Conventional Commits are enforced with `@commitlint/cli` and
`@commitlint/config-conventional`:

- A Husky `commit-msg` hook rejects non-conventional messages locally.
- A `commitlint` job in `quality.yml` validates every commit in each pull
  request range.
- The configured `type-enum` matches the types mapped in
  `release-please-config.json`, so every accepted message maps to a changelog
  section or to "no release" for maintenance work.

### Release binaries

The `Build release binaries` workflow (`build-release.yml`) triggers on
`release: published` and builds native binaries with the official
`tauri-apps/tauri-action` on a four-target matrix: macOS Apple Silicon,
macOS Intel, Windows (NSIS), and Linux (deb, rpm, AppImage). Assets are
attached to the existing release created by release-please, and a final job
uploads a combined `SHA256SUMS.txt`. macOS binaries are ad-hoc signed via
`bundle.macOS.signingIdentity`; full code-signing with notarization requires
Apple Developer credentials and certificate import steps to be added
deliberately.

## Consequences

- Version numbers, `CHANGELOG.md`, release tags, and GitHub Releases are
  produced from Conventional Commits without any manual version editing.
- The startup splash and the Options menu display the exact version the
  binary is built with.
- The first release (`v1.0.0`) is still prepared through the manual release
  process; the automation takes over for every release after the `v1.0.0`
  tag exists on `main`.
- The release-please release pull request is subject to branch protection and
  requires human review before merging, preserving the approval gates in
  `docs/RELEASE.md`.
- Version metadata lives on `main` only; the existing step that synchronizes
  release metadata back into `develop` remains necessary.
- GitHub Actions now requires `contents: write` and `pull-requests: write`
  permissions for the release workflow.
- A structural runtime dependency is not added: release-please is
  configuration and a GitHub Action, not a crate or npm package in the
  application.
