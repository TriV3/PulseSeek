# ADR 0004: Test-Driven Development

- Status: Accepted
- Date: 2026-07-25

## Context

Playback state, filesystem safety, migrations, and cross-manager references are
high-risk behaviors. Technical uncertainty must still produce reviewable,
versioned outcomes.

## Decision

Use strict Red → Green → Refactor for Rust domain and application behavior.
Test React through user-visible behavior. Use real temporary SQLite databases
and small handwritten fakes.

Technical investigations use bounded feature PRs. Any code merged from an
investigation follows the same TDD requirements as other production code.

## Consequences

- Behavior and failure handling are specified before implementation.
- Refactoring has stronger safety.
- Production work begins more deliberately.
- Technical investigations remain possible without lowering production
  standards.
