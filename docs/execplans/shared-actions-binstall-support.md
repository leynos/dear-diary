# Adopt shared-actions stage-release-artefacts for cargo-binstall

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: IN PROGRESS

## Purpose / big picture

The release workflow currently rolls its own logic for staging the release
binary, writing SHA-256 sidecars, and creating the `cargo-binstall` tarball.
That logic lives in `scripts/release_packaging.py` and is invoked by
`scripts/release_support.py prepare-artifact`. Upstream
`leynos/shared-actions` recently introduced a composite action,
`stage-release-artefacts`, that performs the same staging and now (since SHA
`eff100c965da05e14fd4e07d7ea518408b312cb8`) creates deterministic
`cargo-binstall` archives directly from a TOML configuration file. The action
ships its own tests, BDD scenarios, and binstall configuration schema, which
removes the need to maintain bespoke Python for each consumer repository.

This change replaces the bespoke staging logic in `dear-diary` with the shared
action and a `.github/release-staging.toml` configuration. After the change, a
contributor running the release workflow on a `v*.*.*` tag will continue to
see release assets named `dear-diary-<os>-<arch>` and, for the
`*-unknown-linux-gnu` targets, a `dear-diary-<version>-<target>.tar.gz`
cargo-binstall archive next to a matching `.sha256` sidecar. They will be
observable as published files on the GitHub release page in exactly the same
form as before; the only externally visible change is the additional release
note line in the workflow log indicating that staging is performed by the
shared composite action.

Internally, the change removes the `cargo_binstall_archive` matrix flag from
the release workflow, retires the `prepare-artifact` Cyclopts subcommand and
its supporting Python module, and replaces them with a small TOML
configuration that the shared action consumes. The release-process
documentation is updated to point at the new configuration file.

## Constraints

- The release workflow must continue to produce the same set of release
  assets, with the same names, for each existing target:
  - `dear-diary-linux-x86_64` and `dear-diary-linux-x86_64.sha256`
  - `dear-diary-linux-aarch64` and `dear-diary-linux-aarch64.sha256`
  - `dear-diary-freebsd-x86_64` and `dear-diary-freebsd-x86_64.sha256`
  - `dear-diary-<version>-x86_64-unknown-linux-gnu.tar.gz` and its `.sha256`
  - `dear-diary-<version>-aarch64-unknown-linux-gnu.tar.gz` and its `.sha256`
- The cargo-binstall archive layout must remain a single binary entry named
  `dear-diary` at the archive root. This is what
  `crates/dear-diary/Cargo.toml`'s `[package.metadata.binstall.overrides]`
  block expects via `bin-dir = "{ bin }{ binary-ext }"` and `pkg-fmt = "tgz"`.
- The release tag verification step (`uv run scripts/release_support.py
  verify-version`) must continue to run before any build artefacts are
  staged, so that mismatched tags still abort the workflow.
- `crates/dear-diary/Cargo.toml` must not be edited by the release workflow
  or any new helper script; the shared action only reads it.
- All shared-actions references in this repository must be pinned to a single
  SHA, `eff100c965da05e14fd4e07d7ea518408b312cb8` (or newer if subsequently
  upgraded by a follow-up plan). Mixing SHAs across `release.yml` and `ci.yml`
  must be avoided; the SHA upgrade for `ci.yml` is in scope precisely so that
  the repository carries one consistent shared-actions reference.
- Scripts touched by this plan must conform to the project's
  `docs/scripting-standards.md` (Cyclopts, Python 3.13, `uv` script header,
  `cuprum` for any new external command execution, `pathlib.Path` for
  filesystem work, tests in `scripts/tests/` mirroring the script name). No
  new script may shell out via `subprocess` or `os.system`.
- Local validation of the new workflow must follow
  `docs/local-validation-of-github-actions-with-act-and-pytest.md` (black-box
  `act` driven from `pytest`, asserting on artefacts and structured logs;
  no host-side command interception inside the matrix-build job).
- The bin name `dear-diary` is treated as a Cargo identifier; it contains a
  hyphen, which means TOML template strings that need it must use the bare
  `{bin_name}` placeholder rather than any shell expansion.

If satisfying the objective requires violating a constraint, do not proceed.
Document the conflict in `Decision Log` and escalate.

## Tolerances (exception triggers)

- Scope: if implementation requires modifying more than 12 files (excluding
  the imported reference docs and the execplan itself) or more than 600 net
  lines of code, stop and escalate.
- Interface: if any release asset name on the GitHub release page would
  change, stop and escalate. The asset list is part of the consumer contract
  for `cargo-binstall` users.
- Dependencies: if the shared action requires Python packages that are not
  already vendored by it (it pulls its own with `uv`), or if a new top-level
  Python dependency must be added to this repository's tests, stop and
  escalate before adding it.
- External tooling: if `act` cannot exercise the new workflow at all (for
  example, the runner image lacks `cross`), stop and document the gap rather
  than rewriting the workflow to suit `act`. Pre-CI validation may use a
  cut-down matrix; record what was skipped.
- Iterations: if integration tests still fail after three full `act` runs in
  a row for the same reason, stop and escalate.
- Time: if any single milestone (B, C, D, or E below) takes more than four
  wall-clock hours of implementation work, stop and escalate.
- Ambiguity: if multiple valid mappings between the existing artefact names
  and the shared action's TOML schema appear and the choice materially
  affects external consumers (asset names, archive member names, sidecar
  digest format), stop and present the options.

## Risks

- Risk: the shared action's TOML schema does not expose enough template
  variables to produce the existing `dear-diary-<os>-<arch>` asset name as
  the destination filename.
  Severity: medium
  Likelihood: low
  Mitigation: the action's documented destination template supports
  `{bin_name}`, `{platform}`, `{arch}`, and `{bin_ext}`, which is sufficient
  to compose `dear-diary-<platform>-<arch>{bin_ext}`. The mitigation is to
  verify this in milestone B before deleting any local code, by running the
  staging script in dry-run mode against the new configuration and reading
  the `staged-files` output.
- Risk: the shared action expects `manifest_path = "Cargo.toml"` at the
  workspace root by default, but `dear-diary`'s installable package is at
  `crates/dear-diary/Cargo.toml`.
  Severity: medium
  Likelihood: high
  Mitigation: explicitly set `manifest_path = "crates/dear-diary/Cargo.toml"`
  in `[common.binstall]`. The action documents this override; failure to set
  it would resolve the workspace package name and version instead.
- Risk: the `cross`-built binary path under
  `target/<target>/release/dear-diary` differs from the action's default
  `binary_source = "target/{target}/release/{bin_name}{bin_ext}"`.
  Severity: low
  Likelihood: low
  Mitigation: the default template already matches the layout. No override
  required, but the validation step must inspect `binary_path` output.
- Risk: changing the `setup-rust` and other shared-action SHA references in
  `ci.yml` to the new SHA causes a CI regression unrelated to this plan
  (toolchain change, etc.).
  Severity: medium
  Likelihood: low
  Mitigation: stage the SHA bump as its own commit so it can be reverted
  independently. Inspect the `setup-rust/TOOLCHAIN_VERSION` between the old
  and new SHAs before pushing; document any change in the Decision Log.
- Risk: `act` cannot run the FreeBSD matrix entry (it has no `cross` image
  that targets `x86_64-unknown-freebsd` end-to-end).
  Severity: low
  Likelihood: high
  Mitigation: the local `act` job exercises a simplified `stage-only`
  workflow that uses a pre-fabricated dummy binary, not `cross build`.
  Production `cross` builds are validated only on GitHub-hosted runners,
  matching the validation ladder in the act/pytest guide.
- Risk: the shared action writes checksum digests with a different format
  (line layout, hash algorithm) than the existing `release_packaging.py`
  output, breaking downstream users who consume the `.sha256` files.
  Severity: medium
  Likelihood: medium
  Mitigation: the action's documented format is `{digest}  {basename}\n`
  using SHA-256, which matches the existing manifest. Verify byte-for-byte
  in milestone B with a fixture binary.
- Risk: a new top-level `.github/release-staging.toml` is misread by other
  workflows or tooling that glob `.toml`.
  Severity: low
  Likelihood: low
  Mitigation: place the file under `.github/` (as the upstream README
  suggests) and document its purpose in `docs/release-process.md`.

Risks differ from Surprises: risks are anticipated; surprises are not.

## Progress

- [x] Milestone A: orient and align (no code changes). Completed
      (2026-06-03 11:31Z).
- [x] Milestone B: add `.github/release-staging.toml` and prove it locally
      against a fixture binary.
      Completed (2026-06-03 11:38Z). Evidence: direct invocation of the
      pinned shared action's `stage.py` against scratch workspace
      `/tmp/dd-stage-4vVPIA` created `dear-diary-linux-x86_64`,
      `dear-diary-linux-x86_64.sha256`,
      `dear-diary-0.1.0-x86_64-unknown-linux-gnu.tar.gz`, and
      `dear-diary-0.1.0-x86_64-unknown-linux-gnu.tar.gz.sha256`; `tar -tzf`
      reported the single member `dear-diary`; `sha256sum -c` passed for both
      sidecars. Full log:
      `/tmp/stage-proof-dear-diary-chore_plan-shared-actions-binstall-update.out`.
- [x] Milestone C: switch `.github/workflows/release.yml` to the shared
      action; unify shared-action SHA across `release.yml` and `ci.yml`.
      Completed (2026-06-03 13:16Z). Evidence: `make check-fmt`,
      `make test-scripts`, `make lint`, `make test`, and `make markdownlint`
      passed. `act push -e tests/fixtures/release.event.json --list` parsed
      the workflow and reported the `build` and `release` jobs.
- [x] Milestone D: retire `prepare-artifact` and `release_packaging.py`,
      including their tests and snapshots.
      Completed (2026-06-03 13:29Z). Evidence: `make check-fmt`,
      `make test-scripts`, `make lint`, `make test`, and `make markdownlint`
      passed. The residue sweep for `release_packaging`, `prepare_artifact`,
      `prepare-artifact`, `binstall`, `write_sha256`, and `ArtifactRequest`
      under `scripts/` returned no matches.
- [ ] Milestone E: add `act`-driven black-box pytest harness under `tests/`,
      with a self-checking selftest workflow plus a job that exercises the
      real release-staging configuration against a dummy binary.
- [ ] Milestone F: refresh `docs/release-process.md` and add a changelog
      entry; update any references in `AGENTS.md` if they mention the
      retired script.

Use timestamps in the form `(YYYY-MM-DD HH:MMZ)` whenever a milestone is
completed or partially completed.

## Surprises & discoveries

- `git ls-remote https://github.com/leynos/shared-actions.git` did not list
  `eff100c965da05e14fd4e07d7ea518408b312cb8` because the commit is not a
  remote ref tip. A shallow clone plus `git checkout` confirmed the commit is
  fetchable and is titled `Add cargo-binstall archive staging support (#270)`.
  Date/Author: 2026-06-03, Codex.

## Decision log

- Decision: import the upstream `scripting-standards.md` from
  `agent-template-python` over the existing local copy.
  Rationale: the local copy still describes `plumbum` as the command-execution
  library, but the upstream guidance has moved to `cuprum` and is the source
  of truth referenced by this plan. The existing scripts in this repository
  do not currently use `plumbum`, so no consumer code is affected by the swap.
  Any future external-command work performed under this plan will use the
  newer guidance.
  Date/Author: 2026-06-02, Codex.
- Decision: keep `scripts/release_support.py verify-version` and
  `scripts/release_version.py` in place rather than folding them into the
  shared action.
  Rationale: the shared action stages already-built binaries and does not
  enforce tag/version equality. The verify-version step is small,
  well-tested, and the only piece of release logic that is genuinely
  project-specific. Replacing it is out of scope.
  Date/Author: 2026-06-02, Codex.
- Decision: target SHA `eff100c965da05e14fd4e07d7ea518408b312cb8` for every
  `leynos/shared-actions` reference in this repository (currently `setup-rust`,
  `generate-coverage`, and `upload-codescene-coverage` in `ci.yml`, and the
  new `stage-release-artefacts` use in `release.yml`).
  Rationale: a single SHA is easier to reason about and to dependabot.
  Date/Author: 2026-06-02, Codex.
- Decision: keep the plan's target keys and TOML field names unchanged after
  reading the pinned shared action.
  Rationale: the action accepts `config-file` and `target`, exposes
  `artifact-dir`, `binary-path`, and `binstall-archive-path`, supports
  `staging_dir_template = "{platform}-{arch}"`, and merges a
  target-specific `[targets.<name>.binstall]` table over `[common.binstall]`.
  No schema ambiguity was found in milestone A.
  Date/Author: 2026-06-03, Codex.
- Decision: explicitly disable cargo-binstall archive creation for
  `targets.freebsd-x86_64`.
  Rationale: today's `crates/dear-diary/Cargo.toml` binstall metadata only
  advertises Linux GNU archives through a Linux-specific override. Publishing
  a FreeBSD tarball without matching package metadata would create an
  unadvertised artefact rather than preserving the current release contract.
  Date/Author: 2026-06-03, Codex.
- Decision: accept the `setup-rust` SHA bump after inspecting the action diff
  between `d400b079fb6a8fa92f7e7b6c57f3d1c92a4b2d54` and
  `eff100c965da05e14fd4e07d7ea518408b312cb8`.
  Rationale: there is no `.github/actions/setup-rust/TOOLCHAIN_VERSION` file
  at either SHA. The action diff is limited to cargo-binstall installer
  hardening: `BINSTALL_VERSION` is exported so the child installer receives the
  pinned tag, and the installed version is checked after installation. No Rust
  toolchain selection changed.
  Date/Author: 2026-06-03, Codex.
- Decision: remove script-level tests for bespoke checksum and tarball
  creation with `scripts/release_packaging.py`.
  Rationale: that behaviour now belongs to the pinned shared action and its
  own test suite. Keeping local unit tests against deleted implementation
  details would either require retaining dead code or re-testing an external
  action through inappropriate script-level fixtures.
  Date/Author: 2026-06-03, Codex.

## Outcomes & retrospective

To be filled in at the end of implementation. At minimum: confirmation that
each constraint above held, the final list of release assets observed on a
test tag, and any drift between the planned tolerances and what was actually
required.

## Context and orientation

The repository is a Rust workspace at the repository root. The installable
binary is the `dear-diary` package at `crates/dear-diary/Cargo.toml`, which
contains a `[package.metadata.binstall.overrides]` block targeting
`cfg(all(target_os = "linux", any(target_arch = "x86_64", target_arch =
"aarch64"), target_env = "gnu"))`. That override expects archives of the
form `{repo}/releases/download/v{version}/{name}-{version}-{target}.tar.gz`
with `pkg-fmt = "tgz"` and `bin-dir = "{bin}{binary-ext}"`. This means each
archive must contain a binary entry named `dear-diary` at the root.

The release workflow lives at `.github/workflows/release.yml`. It triggers on
tags matching `v*.*.*`, then runs a matrix `build` job per `(os, arch,
target)` triple. The matrix carries a `cargo_binstall_archive` boolean
indicating whether the entry should produce a binstall tarball; today that is
`true` for the two Linux gnu targets and `false` for FreeBSD. The build job
checks out the repo, sets up Python and `uv`, verifies the tag against
`Cargo.toml`, sets up Rust via
`leynos/shared-actions/.github/actions/setup-rust@d400b079fb6a8fa92f7e7b6c57f3d1c92a4b2d54`,
installs `mold`, installs `cross` from a pinned git rev, restores the cargo
cache, builds the release binary with `cross`, and finally invokes
`uv run scripts/release_support.py prepare-artifact` to copy the binary into
`artifacts/<os>-<arch>/`, write its `.sha256` sidecar, and (when
`cargo_binstall_archive` is true) write the binstall tarball with a matching
`.sha256`. A second `release` job downloads every artefact directory and
uploads everything to the GitHub release via `gh release upload --clobber`.

`scripts/release_packaging.py` is the bespoke module under attack. It owns
`ArtifactRequest`, `write_sha256_manifest`, and `prepare_artifacts`. It is
tested by `scripts/tests/test_release_support.py`, which uses the syrupy
snapshot store at `scripts/tests/__snapshots__`. The Cyclopts command
`prepare_artifact` in `scripts/release_support.py` is the only caller of
`prepare_artifacts`; if `prepare_artifacts` is removed, the command must go
with it.

`scripts/release_version.py` and the `verify_version` Cyclopts command stay
as they are. They are the only Python that survives this change.

The CI workflow at `.github/workflows/ci.yml` references the same
`shared-actions` SHA (`d400b079...`) three times: for `setup-rust`,
`generate-coverage`, and `upload-codescene-coverage`. This plan bumps those
to `eff100c9...` so the repository carries one consistent SHA.

The shared action this plan adopts is
`leynos/shared-actions/.github/actions/stage-release-artefacts@eff100c965da05e14fd4e07d7ea518408b312cb8`.
Its README documents two halves: a generic artefact staging mechanism (input
`config-file` plus `target`, output `binary-path` and `staged-files`), and a
new opt-in cargo-binstall archive feature controlled by a
`[common.binstall]` or `[targets.<name>.binstall]` table. The action exports
`binstall-archive-path` when the feature is enabled. The archive name
defaults to `{package_name}-{version}-{target}.tar.gz`, which is exactly the
naming convention this repository uses today, so no override is required.
The archive member name defaults to `{bin_name}{bin_ext}`, which yields
`dear-diary` on Linux. The archive's `.sha256` sidecar is written next to it.

Two reference documents have been imported into `docs/` prior to drafting
this plan:

- `docs/local-validation-of-github-actions-with-act-and-pytest.md` describes
  the black-box approach to local workflow validation used in milestone E.
- `docs/scripting-standards.md` (overwritten with the upstream version)
  describes the Cyclopts/cuprum/pathlib standards any new helper script must
  follow.

Terminology used below:

- *staging configuration*: the new file `.github/release-staging.toml`.
- *stage*: the act of copying a built binary into a versioned artefacts
  directory and emitting checksum sidecars.
- *binstall archive*: a `tar.gz` of the binary in the layout consumed by
  `cargo-binstall` per the package metadata.
- *selftest workflow*: a tiny workflow used only by the `act` pytest harness
  to exercise the staging action without invoking `cross`.

## Plan of work

The work proceeds in six milestones with explicit go/no-go points. Each
milestone ends with a validation step; later milestones do not begin until
the previous one validates.

### Milestone A: orient and align (no code changes)

Read the shared action's README and the binstall test job in its workflow
(`leynos/shared-actions/.github/workflows/test-stage-release-artefacts.yml`,
job `test-stage-artefacts-binstall`) to confirm the exact TOML schema and
the exact outputs available. Confirm the inputs to `stage-release-artefacts`
match what this repository can supply: a TOML config file path, a target
key, and (optionally) the Windows-path-normalisation flag. Confirm the
release workflow's existing matrix variables can map cleanly to a target
table per matrix row.

Validation: write a one-paragraph note in the Decision Log if any field in
the schema is ambiguous. Otherwise mark milestone A complete in `Progress`.

### Milestone B: introduce `.github/release-staging.toml` and prove it locally

Create `.github/release-staging.toml` with one `[common]` table, one
`[common.binstall]` table whose `enabled = true`, and three `[targets.*]`
tables matching today's matrix. Use exactly these target keys, matching the
existing `<os>-<arch>` artefact directory naming so the workflow can pass
the matrix row's key straight through:

- `linux-x86_64` with `platform = "linux"`, `arch = "x86_64"`,
  `target = "x86_64-unknown-linux-gnu"`.
- `linux-aarch64` with `platform = "linux"`, `arch = "aarch64"`,
  `target = "aarch64-unknown-linux-gnu"`.
- `freebsd-x86_64` with `platform = "freebsd"`, `arch = "x86_64"`,
  `target = "x86_64-unknown-freebsd"`.

In `[common]` set:

- `bin_name = "dear-diary"`
- `dist_dir = "artifacts"`
- `checksum_algorithm = "sha256"`
- `staging_dir_template = "{platform}-{arch}"` so the staging directory
  becomes `artifacts/linux-x86_64`, matching today's `dist_dir`/`os-arch`
  convention used by the upload step in `release.yml`.

Add a single `[[common.artefacts]]` entry that stages the release binary
with the existing renamed filename:

```toml
[[common.artefacts]]
source = "target/{target}/release/{bin_name}{bin_ext}"
destination = "{bin_name}-{platform}-{arch}{bin_ext}"
output = "binary_path"
```

In `[common.binstall]` set:

- `enabled = true`
- `manifest_path = "crates/dear-diary/Cargo.toml"`
- All other fields left at their defaults (archive name template,
  binary source template, binary name template, and output key).

Override `enabled = false` for the FreeBSD target by adding a
`[targets.freebsd-x86_64.binstall]` table that explicitly opts out:

```toml
[targets.freebsd-x86_64.binstall]
enabled = false
```

This mirrors the boolean `cargo_binstall_archive` flag that today's matrix
carries.

Validate the staging configuration locally before touching the workflow:

1. Fabricate a fake `cross` output: create a placeholder
   `target/x86_64-unknown-linux-gnu/release/dear-diary` with executable bits
   set and some predictable content.
2. Clone the shared-actions repo at the pinned SHA into a scratch directory
   and run the staging script directly on the fake binary:

   ```bash
   uv run /path/to/shared-actions/.github/actions/stage-release-artefacts/scripts/stage.py
   ```

   with the relevant `INPUT_*` environment variables and
   `TEST_WORKSPACE=$(pwd)`.
3. Confirm the resulting tree:
   - `artifacts/linux-x86_64/dear-diary-linux-x86_64`
   - `artifacts/linux-x86_64/dear-diary-linux-x86_64.sha256`
   - `artifacts/linux-x86_64/dear-diary-<version>-x86_64-unknown-linux-gnu.tar.gz`
   - `artifacts/linux-x86_64/dear-diary-<version>-x86_64-unknown-linux-gnu.tar.gz.sha256`
   where `<version>` is resolved from `crates/dear-diary/Cargo.toml` (the
   workspace version is inherited).
4. Untar the archive and confirm exactly one member named `dear-diary`.
5. Compare the `.sha256` byte stream against a manually computed SHA-256 to
   confirm format compatibility.

If any of these checks fail, do not advance to milestone C. Record what
failed in `Surprises & Discoveries` and discuss the route forward in the
Decision Log.

### Milestone C: switch the workflow to the shared action

Edit `.github/workflows/release.yml`:

1. In the matrix, drop the `cargo_binstall_archive` field; the staging
   configuration now owns that decision.
2. Add a `key` field to each matrix entry whose value matches the target key
   in the staging TOML (`linux-x86_64`, `linux-aarch64`, `freebsd-x86_64`).
3. Remove the `Prepare artifact` step that invokes
   `uv run scripts/release_support.py prepare-artifact`.
4. Add a `Stage release artefacts` step immediately before the upload step,
   using the shared action pinned to
   `eff100c965da05e14fd4e07d7ea518408b312cb8`. Pass `.github/release-staging.toml`
   as `config-file` and `${{ matrix.key }}` as `target`.
5. Keep the `Upload release artifact` step but change its `path:` to
   `${{ steps.stage.outputs.artifact-dir }}` so it does not assume the
   layout. Keep its `name:` as `${{ env.REPO_NAME }}-${{ matrix.os }}-${{
   matrix.arch }}` for backward compatibility with the `release` job that
   downloads under `artifacts/${{ env.REPO_NAME }}-*`.

Bump the three `setup-rust`, `generate-coverage`, and
`upload-codescene-coverage` references in `.github/workflows/ci.yml` from
`d400b079fb6a8fa92f7e7b6c57f3d1c92a4b2d54` to
`eff100c965da05e14fd4e07d7ea518408b312cb8` so the repository carries one
consistent shared-actions SHA. This bump is a separate commit so it can be
reverted independently if it surfaces an unrelated regression.

Validation: run `make check-fmt`, `make lint`, and `make test-scripts`
locally. Verify the workflow file parses correctly with `act --list` against
a tag-push event payload at `tests/fixtures/release.event.json`.

### Milestone D: retire the bespoke staging code

Delete `scripts/release_packaging.py` and remove the `prepare_artifact`
Cyclopts command from `scripts/release_support.py`, including its imports of
`ArtifactRequest` and `prepare_artifacts`. Remove the corresponding tests
under `scripts/tests/test_release_support.py` and the affected snapshots
under `scripts/tests/__snapshots__`. `scripts/release_support.py` should
contain only the `verify_version` command after this milestone.

If `scripts/tests/test_build_configuration.py` imports or asserts anything
from `release_packaging`, update or remove the affected tests with the
rationale recorded in the Decision Log.

Validation: `make test-scripts` passes, `make lint` passes, and the
`scripts/` directory contains no remaining reference to the word
`binstall` or to `release_packaging`. If `rg release_packaging` reports any
hit other than git history, stop and remove the residue.

### Milestone E: act-based black-box pytest harness

Following `docs/local-validation-of-github-actions-with-act-and-pytest.md`,
add a black-box validation harness driven by `pytest`:

1. Add a selftest workflow `.github/workflows/selftest-staging.yml` that has
   one job, `stage-only`, which does *not* invoke `cross`. Instead it
   fabricates a deterministic `target/x86_64-unknown-linux-gnu/release/dear-diary`
   binary, then invokes the shared `stage-release-artefacts` action with
   `.github/release-staging.toml` and target `linux-x86_64`, then uploads the
   resulting `artifacts/linux-x86_64/` directory as an `act`-visible artefact
   named `selftest-stage`. The job emits structured log lines on standard
   output (for example, `metric=selftest_stage_complete target=linux-x86_64`)
   so the pytest harness can grep for them in the `act --json` stream.
2. Add `tests/fixtures/selftest.event.json` with a minimal `pull_request`
   payload, and `tests/fixtures/release.event.json` with a minimal tag-push
   payload for the real workflow.
3. Add `tests/conftest.py` that loads `cmd_mox.pytest_plugin` and exposes a
   shared `act_runner` fixture mirroring the `run_act` helper in the
   reference document. The fixture must read its image override from the
   environment variable `ACT_RUNNER_IMAGE` and default to
   `catthehacker/ubuntu:act-latest`.
4. Add `tests/test_selftest_stage.py` with two tests:
   - `test_stage_only_produces_expected_artefacts` asserts that after
     `act pull_request -j stage-only`, the artefact server directory contains
     `dear-diary-linux-x86_64`, `dear-diary-linux-x86_64.sha256`, the binstall
     tarball, and the tarball's `.sha256`. The tarball is opened in place and
     its single member is asserted to be `dear-diary`.
   - `test_stage_only_emits_metric` asserts that the structured log stream
     contains the `metric=selftest_stage_complete` line for the staged
     target.
5. Add `tests/test_release_workflow_dry_run.py` with a single test that
   invokes `act push -j build -e tests/fixtures/release.event.json --list`
   and asserts that the action graph resolves without errors and includes a
   step whose name is `Stage release artefacts`. This is a parse-time smoke
   test only; the real `cross` build is not exercised locally.
6. Add a `make test-workflow` target that runs
   `uv run --with pytest --with cmd-mox python -m pytest tests/` and is
   listed in `make help`. It is not part of `make all` because it requires
   Docker and `act`; document this in the Makefile's recipe.

Validation: `make test-workflow` passes locally on the development host.
Record `act --version` and the runner image SHA in the Decision Log so
future re-runs can reproduce the environment.

### Milestone F: documentation and changelog

Edit `docs/release-process.md` to:

- Replace the description of `release.yml` with one that names
  `.github/release-staging.toml` as the source of truth for staging, names
  `stage-release-artefacts` as the action that performs the work, and notes
  the pinned SHA.
- Replace the paragraph describing bespoke checksum and tarball creation
  with a short summary pointing to the shared action's README.
- Add a paragraph describing the local validation ladder
  (`make test-workflow` -> GitHub-hosted CI -> published release page),
  cross-linking to `docs/local-validation-of-github-actions-with-act-and-pytest.md`.

Add a changelog or release-notes entry, if the repository carries one
(check at implementation time). Update `AGENTS.md` only if it references the
retired `prepare-artifact` command.

Validation: `markdownlint-cli2 '**/*.md'` (via the CI invocation) is clean,
and the documents read coherently when followed by a new contributor.

## Concrete steps

The exact commands to run, with expected outputs, are recorded below. Each
command's working directory is the repository root unless otherwise stated.
Update this section as work proceeds; each completed command should keep its
expected transcript so a future reader can audit.

```bash
# Sanity-check the shared action SHA before pinning anywhere.
git ls-remote https://github.com/leynos/shared-actions.git \
  | grep eff100c965da05e14fd4e07d7ea518408b312cb8
# Expected: at least one ref line referring to that SHA.
```

```bash
# Milestone B local proof. The TEST_WORKSPACE points at a scratch dir
# populated with the fake binary and the staging TOML.
mkdir -p /tmp/dd-stage/target/x86_64-unknown-linux-gnu/release
printf 'fake-binary\n' \
  > /tmp/dd-stage/target/x86_64-unknown-linux-gnu/release/dear-diary
chmod +x /tmp/dd-stage/target/x86_64-unknown-linux-gnu/release/dear-diary
cp .github/release-staging.toml /tmp/dd-stage/test-staging.toml
cp crates/dear-diary/Cargo.toml /tmp/dd-stage/Cargo.toml

TEST_WORKSPACE=/tmp/dd-stage \
INPUT_CONFIG_FILE=/tmp/dd-stage/test-staging.toml \
INPUT_TARGET=linux-x86_64 \
INPUT_NORMALIZE_WINDOWS_PATHS=false \
INPUT_PS_MODULE_NAME='' \
uv run /path/to/shared-actions/.github/actions/stage-release-artefacts/scripts/stage.py

# Expected directory tree under /tmp/dd-stage/artifacts/linux-x86_64:
#   dear-diary-linux-x86_64
#   dear-diary-linux-x86_64.sha256
#   dear-diary-<version>-x86_64-unknown-linux-gnu.tar.gz
#   dear-diary-<version>-x86_64-unknown-linux-gnu.tar.gz.sha256
```

```bash
# Milestone C parse-time check.
act push -e tests/fixtures/release.event.json --list
# Expected: a job graph listing build and release jobs without errors.
```

```bash
# Milestone D regression sweep.
make test-scripts
rg --files-with-matches release_packaging scripts || echo 'clean'
# Expected: pytest passes; the rg sweep prints exactly 'clean'.
```

```bash
# Milestone E local validation.
make test-workflow
# Expected: two pytest cases pass; the act --json stream contains the
# metric=selftest_stage_complete line and the artefact tree matches.
```

## Validation and acceptance

Acceptance is observed at three levels:

1. **Local (pre-CI)**: `make all` passes (`check-fmt`, `lint`, `test`,
   `test-scripts`), and `make test-workflow` passes. The latter is gated on
   Docker and `act` being installed and is not part of `make all`.
2. **CI on a feature branch**: pushing the branch with `[release-dry-run]`
   in the commit subject (or via a temporary dispatch trigger in the
   selftest workflow) succeeds on GitHub-hosted runners and produces the
   `selftest-stage` artefact for inspection.
3. **Tagged release**: tagging a throwaway `v0.0.0-pre.N` commit on a fork
   triggers `release.yml` end-to-end. The GitHub release page must list
   exactly the eight assets enumerated under `Constraints`. A reviewer
   downloads `dear-diary-linux-x86_64.sha256`, runs `sha256sum -c`, and
   confirms a clean match.

Quality criteria (what "done" means):

- Tests: `make test-scripts` and `make test-workflow` pass.
- Lint and types: `make lint` passes, including markdownlint of the new
  TOML- and doc-adjacent files.
- Performance: not in scope; staging cost is bounded by file copy time and
  one SHA-256 pass per artefact.
- Security: no script reads or writes a path outside the workspace; the
  shared action is pinned by full SHA.

Quality method (how we check): run the local commands listed above and
confirm the CI workflow on the feature branch finishes green before merge.

## Idempotence and recovery

Re-running the release workflow on the same tag re-stages identical files
into the same directories and overwrites them. The shared action's checksum
sidecars are byte-identical for byte-identical inputs, so reruns produce no
drift.

If a release run partially uploads assets and then fails, the `gh release
upload --clobber` invocation already overwrites on retry; no manual cleanup
is required.

If milestone B reveals that the staging TOML schema cannot produce the
required asset names, stop. Do not edit `release.yml` to compensate; revisit
the schema with the action's maintainer and update this plan's Decision
Log.

If milestone E reveals that `act` cannot reproduce a step that the GitHub
runner handles, document the gap in `Surprises & Discoveries` and rely on
the GitHub-hosted validation ladder for that path only.

## Artifacts and notes

- Reference documents imported into `docs/` before drafting this plan:
  - `docs/local-validation-of-github-actions-with-act-and-pytest.md`
  - `docs/scripting-standards.md` (overwritten with the upstream version)
- Shared action under test:
  `leynos/shared-actions/.github/actions/stage-release-artefacts@eff100c965da05e14fd4e07d7ea518408b312cb8`
- Existing local staging code that this plan retires:
  - `scripts/release_packaging.py`
  - the `prepare_artifact` command in `scripts/release_support.py`
  - the matching tests in `scripts/tests/test_release_support.py` and any
    snapshots in `scripts/tests/__snapshots__`

## Interfaces and dependencies

The new file `.github/release-staging.toml` is the single source of truth
for what gets staged. Its schema is the one documented in the
`stage-release-artefacts` README at the pinned SHA. The action's outputs
this plan consumes are:

- `artifact-dir` (absolute path used by the upload step's `path:`).
- `binary-path` (used by validation tests to confirm the binary was
  renamed correctly).
- `binstall-archive-path` (used by validation tests to assert the tarball
  exists for Linux gnu targets and is empty for FreeBSD).

After this plan, `scripts/release_support.py` exposes only the
`verify_version` Cyclopts command. Its signature is unchanged:

```python
@app.command
def verify_version(
    *,
    cargo_toml_path: Annotated[Path, Parameter(required=True)],
    github_ref_name: Annotated[str, Parameter(required=True)],
    project_root: Path = Path("."),
) -> None: ...
```

No new external dependency is added to the repository's scripts. The
`tests/` harness uses `pytest`, `cmd-mox`, and Docker via `act`; these are
declared in the `make test-workflow` recipe rather than added to a
top-level `pyproject.toml`, to keep the dependency surface small. If a
future plan introduces a long-lived Python project layout, the harness can
be folded into it; that is out of scope here.
