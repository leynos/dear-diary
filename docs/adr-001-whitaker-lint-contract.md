# ADR 001: Adopt Whitaker as part of the lint contract

## Status

Accepted.

## Context

Dear Diary already treats `make lint` as a required commit gate. Before this
decision, that target covered Rustdoc and Clippy, but it did not enforce
project-specific architecture and maintainability rules such as module-level
documentation, non-panicking shared helpers, capability-oriented filesystem
access, or Bumpy Road complexity limits.

The collection-name interpolation path is a representative risk area. It runs
during configuration loading, touches git state through an injected
`GitContext`, parses remote URL shapes, and maps failures into domain errors.
That code should remain explicit and testable, but clustered branching can make
startup configuration behaviour difficult to review.

## Decision

The project adopts Whitaker as part of the local and CI lint contract. The
`make lint` target runs Rustdoc, Clippy, and Whitaker. Continuous Integration
(CI) installs the pinned Whitaker installer revision and runs
`whitaker-installer --cranelift` immediately before `make lint`, matching the
same contract contributors run locally.

Whitaker findings are treated as design feedback. When the suite reports
complexity in production code, the preferred response is to extract named
helpers that keep responsibilities separate. In the interpolation path, this
means preserving `GitContext` as the git boundary, keeping remote URL parsing
in the parser module, and returning typed `ConfigError` values instead of
panicking or hiding fallibility.

## Consequences

- Pull requests must keep Rustdoc, Clippy, and Whitaker green before review.
- CI takes on an additional setup step for the pinned Whitaker installer.
- Refactors driven by Bumpy Road findings should favour small private helpers
  over lint suppressions.
- Test helpers must avoid `.expect()` outside recognised test bodies; when a
  helper can fail, it should return `Result` or keep the failure inside the
  test body.
- Configuration-loading tests should cover both isolated interpolation helpers
  and the `Settings::from_env_with_git()` workflow that wires environment
  variables into interpolation.

## Alternatives considered

The project could have left Whitaker as an optional local tool. That would have
kept CI faster, but it would also allow architecture and complexity regressions
to reach review without a deterministic gate.

The project could have suppressed the first findings and limited this change to
tool installation. That would have enabled the command path, but it would have
weakened the purpose of adding Whitaker. Fixing the interpolation and test
hygiene findings makes the new lint contract active immediately.
