/**
 * Commit message rules for PulseSeek.
 *
 * Conventional Commits feed release-please, which derives the version bump
 * and the CHANGELOG.md entries from commit types. Keep the commit subjects
 * user-visible and precise; see docs/RELEASE.md for the changelog mapping.
 */
export default {
  extends: ["@commitlint/config-conventional"],
  rules: {
    "type-enum": [
      2,
      "always",
      [
        "build",
        "chore",
        "ci",
        "docs",
        "feat",
        "fix",
        "perf",
        "refactor",
        "revert",
        "style",
        "test",
      ],
    ],
    "subject-case": [0],
    "scope-case": [0],
  },
};
