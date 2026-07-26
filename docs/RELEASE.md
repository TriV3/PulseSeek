# Release Process

> Playbook for preparing and publishing PulseSeek desktop releases.
>
> Read this document before every release. A release changes public repository
> state and must never be started, published, or merged without the repository
> owner's explicit approval.

## Overview

PulseSeek follows Semantic Versioning and GitFlow:

- `develop` is the integration branch.
- `release/<version>` branches are created from `develop`.
- Release pull requests target `main`.
- `main` contains released or immediately releasable versions.
- Release tags use the form `vMAJOR.MINOR.PATCH`.
- Release changes are synchronized back into `develop` through a pull request.

Before `1.0.0`, minor versions may introduce significant product changes and
patch versions contain compatible fixes or small improvements. Every release
must explain any compatibility or data-migration impact explicitly.

PulseSeek is a desktop application. Do not run `npm publish`: releases consist
of versioned source, GitHub release notes, and verified desktop artifacts.

## `/release` contract

The command:

```text
/release <VERSION>
```

starts local preparation of the specified release. The agent must immediately
read this document, validate the requested Semantic Version against the changes
since the latest tag, create and switch to the local `release/<VERSION>`
branch, and begin the non-destructive preparation steps.

If `<VERSION>` is omitted, the agent must analyze the release range, propose a
version with its reasoning, and wait for approval before creating a release
branch. Invoking `/release <VERSION>` authorizes that local branch creation but
does not authorize any public action listed below.

## Release authority

An agent may inspect history, propose a version, draft release notes, and run
local validation without additional permission.

Explicit user approval is required before:

- Pushing a release branch.
- Editing accepted version metadata for publication.
- Opening, retargeting, merging, or closing a release pull request.
- Creating or pushing a tag.
- Creating, editing, or publishing a GitHub Release.
- Uploading binaries or other release artifacts.
- Temporarily changing branch protections or repository permissions.

Never disable or bypass branch protection merely to publish a release.

## Changelog format

`CHANGELOG.md` is the human-readable release history. Each published version
uses this structure:

```markdown
## [0.2.0] - 2026-08-15

### Added

- **Folder auditioning** — Preview supported audio files directly from the
  folder tree.

### Changed

- **Faster waveform display** — Show the first useful waveform preview sooner.

### Fixed

- **Output-device recovery** — Continue playback correctly after reconnecting
  an audio interface.
```

Use only the applicable categories:

| Category | Content |
| --- | --- |
| **Added** | New user-visible capabilities |
| **Changed** | Improvements or intentional behavior changes |
| **Fixed** | Corrected defects and regressions |
| **Deprecated** | Features planned for removal |
| **Removed** | Removed capabilities or compatibility |
| **Security** | Security and privacy corrections |

Changelog rules:

- Lead with the user-visible outcome, not the implementation.
- Use one bullet per distinct change.
- Begin each bullet with a concise bold lead-in followed by ` — `.
- Omit empty categories.
- Keep ordinary bullets concise; add detail only for migration or safety.
- Exclude internal refactors unless users, integrators, or plugin authors are
  affected.
- Mark breaking changes clearly.
- Link migration instructions or ADRs when a release changes stored data,
  plugin contracts, or supported platforms.

## Phase 1: Establish the release

### 1. Confirm a clean starting point

Start from an up-to-date `develop` with no unrelated local changes:

```bash
git status --short --branch
git fetch origin
git log --oneline --decorate origin/develop..develop
git log --oneline --decorate develop..origin/develop
```

Do not reset, clean, stash, or overwrite local work automatically. If the
working tree is not safe, stop and report it.

### 2. Determine the release range

Find the most recent version tag:

```bash
git tag --list 'v[0-9]*' --sort=-version:refname
```

The release range is:

- `<LAST_TAG>..origin/develop` when a previous release exists.
- The complete reachable history when preparing the first release.

Inspect commits, merged pull requests, and their actual diffs. Do not generate
release notes from commit subjects alone.

### 3. Propose the version

Use Semantic Versioning:

- **Patch:** compatible bug fixes and very small compatible improvements.
- **Minor:** new compatible capabilities.
- **Major:** incompatible public behavior, data, plugin, IPC, or integration
  contract changes.

Before `1.0.0`, explain why a change belongs in the proposed minor or patch
version. Present the proposed version and reasoning to the user and wait for
approval before creating the release branch.

### 4. Create the release branch

After approval:

```bash
git switch develop
git pull --ff-only origin develop
git switch -c release/<VERSION>
```

Creating the branch locally is not permission to push it.

## Phase 2: Draft and validate release notes

### 5. Analyze every included change

For each commit or merged PR in the release range:

1. Read the complete diff.
2. Inspect the previous version of affected behavior when necessary.
3. Identify the user-visible outcome.
4. Record compatibility, migration, privacy, audio, and data-safety impact.
5. Trace every changelog claim to the reviewed change.

If the history is large, group related implementation commits by user-visible
outcome. Never hide a breaking or safety-relevant change inside a generic
summary.

### 6. Draft the changelog entry

Use the intended release date. If the date is not yet known, use
`Unreleased` until publication:

```markdown
## [<VERSION>] - Unreleased
```

Cross-check the draft against previous entries for headings, spacing, bullet
style, and level of detail.

### 7. Present the draft

Show the complete proposed entry to the user before editing `CHANGELOG.md`.
Explain:

- The proposed version.
- The included release range.
- Any breaking change or migration.
- Any known limitation.
- Any release artifact or platform not yet available.

Wait for explicit approval or requested revisions.

### 8. Update release metadata

Once the changelog is approved, update every version source that actually
exists. Depending on the implemented project phase, these may include:

- Rust workspace packages in `Cargo.toml`.
- The frontend package in `package.json`.
- The Tauri application version in `src-tauri/tauri.conf.json`.
- Versioned IPC, plugin, database, or DAW bridge contracts when intentionally
  changed.
- `CHANGELOG.md`.

All application-facing version values must agree. Do not change a protocol or
schema version merely because the application version changed.

During the current documentation-only phase, report missing version files and
skip the release rather than creating placeholder release metadata.

## Phase 3: Verify the release candidate

### 9. Run automated validation

Run every applicable command that exists:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
pnpm format:check
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

Also run repository security and license checks when configured, including
`cargo deny check`.

Do not hide validation failures with output filters. Record commands that are
unavailable, skipped, or not applicable; never claim they passed.

### 10. Build release artifacts

When the Tauri application exists, create production artifacts using the
versioned project command or:

```bash
pnpm tauri build
```

Verify every intended target separately. The target release plan currently
prioritizes:

1. macOS on Apple Silicon.
2. Universal macOS or a separate Intel artifact.
3. Windows.
4. Linux.

For every artifact:

- Confirm the version displayed by the application.
- Launch it on the intended architecture.
- Open a folder and preview supported audio.
- Verify output-device selection.
- Verify light, dark, system, Midnight Blue, and High Contrast themes.
- Confirm that browsing does not import files.
- Confirm that logs do not expose private paths by default.
- Record the checksum.

Do not label an artifact as signed or notarized unless signature and
notarization verification actually passed. Never expose signing credentials in
logs, commits, release notes, or agent output.

### 11. Exercise migration and recovery

When databases or stored configuration exist:

- Test a clean installation.
- Test upgrading from the previous released version.
- Verify database backups before destructive migrations.
- Verify safe behavior with a missing or damaged manager database.
- Confirm that the Audio Player still works independently.
- Document rollback limitations.

### 12. Commit the release candidate

After validation:

```bash
git add CHANGELOG.md Cargo.toml Cargo.lock package.json pnpm-lock.yaml src-tauri/tauri.conf.json
git commit -m "chore(release): prepare v<VERSION>"
```

Stage only files that exist and belong to the approved release. Inspect the
staged diff before committing. Never add `spec/`.

## Phase 4: Release pull request

### 13. Push and open the release PR

With explicit user approval:

```bash
git push -u origin release/<VERSION>
gh pr create \
  --repo TriV3/PulseSeek \
  --base main \
  --head release/<VERSION> \
  --title "chore(release): v<VERSION>"
```

The PR description must contain:

- The complete release summary.
- The release range.
- Automated validation results.
- Manual test results by platform.
- Artifact, signing, and notarization status.
- Migration and rollback notes.
- Known limitations.

### 14. Review the release PR

Run `/review <PR_NUMBER>` and follow `docs/PR-REVIEW.md`. Re-run validation on
the final reviewed commit. If the release branch changes afterward, repeat the
affected review and validation.

### 15. Merge into `main`

Merge only after the user explicitly approves publication and all required
checks pass. Use the merge method permitted by the protected branch and the
repository's release history.

Do not tag the release branch before merging. The release tag must identify the
exact commit that becomes the released `main` commit.

## Phase 5: Tag and publish

### 16. Verify the merge commit

After the release PR is merged:

```bash
git fetch origin
git switch main
git pull --ff-only origin main
git log -1 --show-signature
```

Confirm that:

- `main` contains the approved changelog and version metadata.
- The checked-out commit is the expected PR result.
- Required GitHub checks succeeded.
- The working tree is clean.

### 17. Create the release tag

With explicit user approval, create an annotated tag on the verified `main`
commit:

```bash
git tag -a v<VERSION> -m "PulseSeek v<VERSION>"
git push origin v<VERSION>
```

Use a signed tag when repository signing is configured and verifiable. Do not
claim an unsigned tag is verified.

### 18. Create the GitHub Release

Draft the GitHub Release first:

```bash
gh release create v<VERSION> \
  --repo TriV3/PulseSeek \
  --draft \
  --title "PulseSeek v<VERSION>" \
  --notes-file <RELEASE_NOTES_FILE>
```

Attach only artifacts built from the tagged commit. Include platform and
architecture in each filename, plus checksum files.

Before publishing the draft, verify:

- Tag and release version match.
- Notes match the approved changelog.
- All promised artifacts are attached.
- Every checksum matches.
- Signing and notarization statements are accurate.
- Installation and known limitations are clear.

Publishing the draft requires a final explicit user approval.

## Phase 6: Synchronize and close

### 19. Synchronize the release back into `develop`

Do not push directly to `develop`. If the release merge created changes not
already present there, create a synchronization branch from `develop`, merge
or cherry-pick only the release metadata as appropriate, and open a PR
targeting `develop`.

```bash
git switch develop
git pull --ff-only origin develop
git switch -c chore/sync-v<VERSION>
git merge --no-ff origin/main
```

Resolve conflicts carefully, run applicable validation, and request approval
before pushing or creating the synchronization PR.

If `develop` already contains the exact release metadata, document that no
synchronization PR is necessary.

### 20. Final verification

Confirm and report:

- The published tag and its commit.
- The GitHub Release URL and publication state.
- Attached artifacts and checksums.
- Signing and notarization status.
- `main` and `develop` synchronization status.
- Remaining release branches.
- Known limitations and follow-up work.

Delete remote or local release branches only with explicit approval or when
the approved merge operation was configured to delete them.

## Failure handling

### Validation fails

Stop the release. Fix the problem through the release branch, repeat affected
tests and builds, update the changelog if user-visible behavior changed, and
obtain approval again when the release candidate materially differs.

### Artifact differs from the tagged commit

Do not publish it. Rebuild from the tag in a clean environment. Never silently
replace an artifact under an existing published version.

### A published release is defective

Do not move or reuse its tag. Prepare a new patch version through a
`hotfix/<short-name>` or appropriate release branch, preserving an auditable
history.

### A release exposes private data or credentials

Stop publication immediately. Remove public artifacts when authorized, rotate
credentials where applicable, and follow GitHub's sensitive-data removal
procedure. Do not merely delete the latest branch and assume the data is gone.

### Branch protection blocks a step

Treat the protection as intentional. Report the blocked operation and use the
required pull-request workflow. Changing protections requires separate,
explicit approval and must not be the default release path.

## First-release checklist

The first PulseSeek release additionally requires:

- A complete license and third-party notices review.
- A documented minimum macOS version.
- Verified application identifier and bundle metadata.
- Application icons and required platform assets.
- Code-signing and notarization decisions.
- Installation and uninstall instructions.
- A privacy statement consistent with actual diagnostics and telemetry.
- A clean-install test on a machine without development tools.
- A tested update strategy, or an explicit statement that automatic updates
  are not yet supported.
- Checksums for every downloadable artifact.
