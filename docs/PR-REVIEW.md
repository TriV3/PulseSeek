# Pull Request Review Workflow

> Review procedure for PulseSeek pull requests, for both humans and coding
> agents.

## Purpose

The `/review <PR_NUMBER>` command starts a structured review of the specified
GitHub pull request. The goal is to find correctness, safety, architecture,
test, accessibility, and maintainability issues before code reaches
`develop`.

A review is read-only by default. It does not authorize changing code,
retargeting the pull request, pushing a branch, or merging.

## Repository policy

- Ordinary pull requests must target `develop`.
- Release pull requests must target `main` from a `release/*` branch.
- Urgent production fixes use a `hotfix/*` branch and follow the documented
  Git workflow.
- Never push directly to `main`.
- Do not push directly to `develop`; changes must arrive through a pull
  request.
- Do not force-push a contributor's branch.
- Keep `spec/` local and ignored. Do not add it to a commit or expose its
  contents during a review.
- Any push, pull-request mutation, or merge requires explicit user approval.

## `/review` contract

When the user invokes `/review <PR_NUMBER>`, the agent must:

1. Read this document and `AGENTS.md` in full.
2. Resolve the pull request in `TriV3/PulseSeek`.
3. Verify its target branch without changing it.
4. Inspect the complete diff and all changed files.
5. Run every applicable, available validation command.
6. Report actionable findings with precise file and line references.
7. State which checks passed, failed, were unavailable, or were skipped.

The agent must not implement fixes or merge the pull request unless the user
subsequently gives explicit approval.

## Phase 1: Gather context

Inspect the pull request metadata before checking out its code:

```bash
gh pr view <PR_NUMBER> \
  --repo TriV3/PulseSeek \
  --json number,title,body,author,baseRefName,headRefName,isDraft,mergeable,reviewDecision,statusCheckRollup
```

Confirm that the target is appropriate:

- An ordinary feature, refactor, documentation, or maintenance PR targets
  `develop`.
- A release PR targets `main`.
- If the target is incorrect, report it as a blocking finding. Do not retarget
  it automatically.

Fetch the pull request into a dedicated local review branch:

```bash
git fetch origin pull/<PR_NUMBER>/head:review/pr-<PR_NUMBER>
git switch review/pr-<PR_NUMBER>
```

Before continuing, confirm that the working tree contains no unrelated local
changes. Never discard, overwrite, stash, or clean user changes without
explicit approval.

## Phase 2: Inspect the change

For an ordinary PR targeting `develop`:

```bash
git fetch origin develop
git diff --stat origin/develop...review/pr-<PR_NUMBER>
git diff --name-status origin/develop...review/pr-<PR_NUMBER>
git diff origin/develop...review/pr-<PR_NUMBER>
```

Use the actual target branch instead of `develop` for a release or hotfix PR.

Read complete changed files when the surrounding context affects the review.
Also inspect related tests, domain contracts, adapters, migrations, events,
and architecture decisions even when they were not modified.

Review in this order:

1. Product requirements and acceptance criteria.
2. Correctness and failure modes.
3. User-data, filesystem, database, and privacy safety.
4. Real-time audio safety.
5. Domain boundaries and ports-and-adapters dependency direction.
6. Tauri boundary validation and Rust/TypeScript contract parity.
7. Tests and evidence of Red → Green → Refactor.
8. UI behavior, keyboard access, and theme compatibility.
9. Diagnostics, performance, maintainability, and dependency impact.

### PulseSeek-specific checks

Flag any change that:

- Imports files merely because the user browsed or played them.
- Makes the Audio Player depend on a manager database.
- couples the Sample, Music, or Playlist Manager domain models.
- Accesses files, databases, or audio devices directly from React.
- Allocates, locks, logs, performs I/O or SQL, or runs analysis in the audio
  callback.
- Permanently deletes audio without a separate explicit action.
- Modifies source audio or embedded metadata silently.
- Hard-codes theme colors in feature components.
- Performs high-frequency visualization work through React renders.
- Introduces a structural dependency without prior approval.
- Changes an accepted architecture decision without an ADR and approval.

## Phase 3: Validate

Run only commands supported by the current project state. Do not claim a check
passed when it was unavailable, skipped, or not run.

### Rust

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

### Frontend

```bash
pnpm format:check
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

Start with focused tests while investigating a finding, then run the complete
applicable suite before recommending a merge. Do not pipe validation output
through filters that hide failure context.

For desktop or audio changes that cannot be fully automated, identify the
required macOS manual checks explicitly.

## Phase 4: Report

List findings first, ordered by severity:

- **P0 — Critical:** data loss, security exposure, or a change that must never
  ship.
- **P1 — High:** broken core behavior, architecture violation, real-time audio
  risk, or likely regression.
- **P2 — Medium:** meaningful edge case, incomplete failure handling,
  accessibility problem, or missing important test.
- **P3 — Low:** localized maintainability or clarity issue worth fixing.

Each finding must include:

- A concise title.
- The affected file and smallest useful line range.
- The concrete scenario that triggers the problem.
- Its user or system impact.
- A clear direction for resolving it.

After the findings, include:

- A short summary of the change.
- Validation results, including unavailable and skipped checks.
- Manual tests still required.
- A final recommendation: `changes required`, `ready after manual testing`, or
  `ready to merge`.

If there are no findings, say so explicitly and still report residual risks and
validation coverage.

## Phase 5: Apply approved fixes

Only proceed when the user explicitly asks for fixes.

Do not amend or force-push the contributor's branch. Create a dedicated branch
from the reviewed pull-request head:

```bash
git switch review/pr-<PR_NUMBER>
git switch -c review/pr-<PR_NUMBER>-fixes
```

Use TDD for every behavioral fix:

1. Add or update a test that demonstrates the defect.
2. Run it and confirm the expected failure.
3. Implement the smallest correction.
4. Run the focused test until it passes.
5. Refactor without changing behavior.
6. Run the complete applicable validation suite.

Present the resulting diff and validation results before requesting permission
to push. If approved, push the fix branch and open a small PR targeting the
original feature branch. After that fix PR is merged, re-review the original
PR because its head has changed.

## Phase 6: Merge

Merge only after the user explicitly requests it and all of the following are
true:

- No P0, P1, or unresolved P2 finding remains.
- Required checks pass.
- Required manual testing is complete or the user explicitly accepts the
  documented residual risk.
- The PR still targets the correct branch.
- The reviewed head commit is still the current PR head.
- Branch protections allow the merge.

Prefer squash merging for ordinary feature PRs to keep each small feature
atomic:

```bash
gh pr merge <PR_NUMBER> \
  --repo TriV3/PulseSeek \
  --squash \
  --delete-branch
```

Never bypass branch protection merely to merge a pull request. After merging,
verify the resulting commit on the target branch and report the confirmed
outcome.

## Common failure cases

### The pull request changed during review

If the head commit changes, stop and review the new diff. Previous findings and
test results may no longer be valid.

### The working tree is dirty

Preserve all existing work. Do not reset, clean, or stash it automatically.
Use a separate Git worktree only after confirming a safe path and ensuring it
will not interfere with another agent or user process.

### Rebase or merge conflicts

Do not rewrite the contributor's history. Report the conflict and propose a
small, explicit resolution plan.

### A validation command is unavailable

Record the command and the reason it could not run. Continue with the remaining
checks, but do not represent the review as fully validated.

### GitHub access is unavailable

Report which metadata, diff, checks, or review state could not be verified.
Never infer the current remote state solely from stale local references.
