# Developer guide

This guide covers release automation and local validation for contributors.

## Build configuration

The workspace uses the pinned Nightly toolchain declared in
`rust-toolchain.toml`. Nightly is required for the Rust 2024 edition and for
the parallel `rustc` frontend.

Development, test, lint, and typecheck builds run on the repository's build
standard, which `.cargo/config.toml` carries and the Makefile composes into the
`RUSTFLAGS` its recipes assign:

- `-Zthreads=8`, the parallel `rustc` frontend, on every platform.
- The `mold` linker on Linux, driven through `clang`.

The pinned toolchain must include these Rust components:

- `rustfmt`
- `clippy`

Install or repair the pinned toolchain using `rust-toolchain.toml` as the
channel source of truth:

```bash
rustup toolchain install
```

### Cranelift exception

Cranelift is not the development-profile codegen backend here, although Weaver
and Gauss use it. It links this workspace only without the parallel frontend:
under `-Zthreads=8` the link fails with undefined `aws_lc_0_45_0_*` symbols from
`aws-lc-sys`. The build standard requires `-Zthreads=8`, and Cranelift is kept
only where the full suite passes, so the backend is dropped rather than the
frontend flag. The evidence, measured on `nightly-2025-12-10` on 2026-09-29 and
kept with the pull request that adopted the standard:

- Cranelift with `-Zthreads=8` fails to link, in two separate runs.
- Cranelift alone links.
- LLVM with `-Zthreads=8` links.

`scripts/tests/test_build_configuration.py` fails if a `codegen-backend` key or
the Cranelift component returns to the configuration. Revisit the exception when
`aws-lc-sys` links under Cranelift with the parallel frontend.

`make lint` runs Rustdoc, Clippy, and Whitaker. Install Whitaker through the
versioned installer from crates.io before running the full lint target locally
(CI pins the same installer version through the `WHITAKER_INSTALLER_VERSION`
environment variable in `.github/workflows/ci.yml`; the suite itself follows
the rolling Whitaker release):

```bash
cargo install --locked whitaker-installer --version 0.2.6
whitaker-installer
```

Whitaker is a Dylint-based lint suite used to catch architectural and code
health regressions that Clippy does not cover. In this workspace it enforces
rules such as module-level documentation, no panicking `expect` calls outside
recognized test bodies, and Bumpy Road complexity checks. Those checks make the
lint target a maintainability gate, not only a syntax or style gate.

The complexity checks are intentionally active for configuration code. When
Whitaker identifies clustered branching, prefer extracting named helpers that
preserve explicit fallibility and dependency injection boundaries. For example,
collection-name interpolation keeps git access behind `GitContext`, while the
remote URL parser owns URL-shape decisions. This keeps configuration loading
testable without allowing startup parsing logic to grow into a single
multi-purpose function.

The architectural decision is recorded in
[`docs/adr-001-whitaker-lint-contract.md`](adr-001-whitaker-lint-contract.md).

### Spelling policy

The lint and Markdown gates enforce British English with Oxford `-ize`
conventions through `make spelling`, which runs a pinned
[`typos-config-builder`](https://github.com/leynos/typos-config-builder) gate.
The gate regenerates `typos.toml` from the live shared estate dictionary and the
`typos.local.toml` overlay on every run, checks maintained Markdown with its
own pinned `typos` binary, and applies the shared phrase corrections that
`typos` cannot express. Because the dictionary is live, `typos.toml` is never
drift-checked in continuous integration (CI); the tracked copy is only a record
of the last generated policy.

`TYPOS_CONFIG_BUILDER_VERSION` in the `Makefile` pins the
`typos-config-builder` release the gate runs (currently `v0.1.3`). Raise it
together with the regenerated `typos.toml`, never on its own.

Add repository-only proper names or quoted upstream terms to
`typos.local.toml`; never edit generated entries in `typos.toml` by hand.

Linux `x86_64-unknown-linux-gnu` builds link through `clang` with `mold`:

```toml
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-Zthreads=8", "-Clink-arg=-fuse-ld=mold"]
```

Install `clang` and `mold` before running local Linux builds that use that host
target. In GitHub Actions, the CI and release workflows install them through the
`setup-rust` action's `install-mold` and `install-clang-lld` inputs, which
install the packages on Linux, fail the job unless `clang`, `ld.lld` and `mold`
resolve on `PATH`, and skip with a notice elsewhere. No workflow carries a
hand-rolled `apt-get` step, and the inputs set no linker flag, so
`.cargo/config.toml` still chooses the linker. `lld` is newly installed here;
nothing selects it.

### CI and coverage

CI measures Rust coverage with the shared `generate-coverage` action. Coverage
is a measurement, so it stays outside the build standard: the action assigns
its own `RUSTFLAGS` without `-Zthreads=8` or `mold`, and `cargo-llvm-cov`
relies on LLVM coverage instrumentation.

Keep that LLVM instrumentation carve-out inside the shared coverage action. Do
not add a workflow-level or step-level `CARGO_PROFILE_DEV_CODEGEN_BACKEND=llvm`
override: that environment can leak into tool installation before coverage
starts.

Any change to `.cargo/config.toml`, `rust-toolchain.toml`, or the build-related
GitHub Actions wiring must include script-test coverage that verifies the
configuration contract. At minimum, tests should cover the selected codegen
absence of a codegen backend, the Linux linker settings, guarded CI
installation of the linkers through `setup-rust`, and the coverage action
carve-out.

## Release workflow

The release workflow is defined in `.github/workflows/release.yml`. It runs
when a tag matching `v<major>.<minor>.<patch>` is pushed.

The workflow builds the `dear-diary` binary package for the configured target
matrix. Linux GNU release targets also produce `cargo-binstall` archives whose
names match the `[package.metadata.binstall]` configuration in
`crates/dear-diary/Cargo.toml`.

The release job declares `contents: write` because it creates or updates the
GitHub release and uploads release assets. Asset uploads use `--clobber` so a
rerun replaces existing files instead of failing on duplicates.

The shared-actions adoption is tracked in
[`docs/execplans/shared-actions-binstall-support.md`](execplans/shared-actions-binstall-support.md).
Consult that ExecPlan before changing release staging, artefact names,
checksum sidecars, or the cargo-binstall archive contract.

## Release helper script

Release version checks live in `scripts/release_version.py` and are exposed by
`scripts/release_support.py verify-version`. Artefact copying, checksum
creation, and cargo-binstall archive generation are delegated to the pinned
`leynos/shared-actions/.github/actions/stage-release-artefacts` composite
action. Its local configuration is `.github/release-staging.toml`.

Keep project-specific checks in scripts and shared release staging in the
shared action. This keeps version validation testable inside this repository
while avoiding bespoke packaging code that must be maintained in every
consumer. Release automation also pins external Git references rather than
using floating tags; for example, the workflow installs `cross` from a fixed
revision and calls shared actions at a fixed commit so reruns use the same
source.

Python release automation follows
[`docs/scripting-standards.md`](scripting-standards.md):

- `uv` executes the script with its declared Python dependencies.
- Cyclopts maps `INPUT_*` workflow environment variables to typed parameters.
- New automation that executes external commands uses `cuprum` rather than
  `subprocess`, `os.system`, or legacy `plumbum` helpers.
- Pure helper functions handle version resolution and tag validation.

Run the script tests with:

```bash
make test-scripts
```

The tests verify workspace-inherited Cargo versions, tag/version matching,
Cyclopts environment mapping, GitHub Actions upload wiring, shared staging
configuration, cargo-binstall metadata, archive layout, and checksum manifests
that contain only asset basenames. Snapshot tests pin the command output shape
for the release helper commands.

GitHub Actions installs `uv` with `astral-sh/setup-uv` before running the
release helper. The helper targets Python 3.13, matching the repository
scripting standards for new automation.

Run the act-backed release workflow harness with:

```bash
make test-workflow
```

This validates the release workflow graph and exercises the shared staging
configuration against a dummy Linux release binary. The harness follows
[`docs/local-validation-of-github-actions-with-act-and-pytest.md`](local-validation-of-github-actions-with-act-and-pytest.md):
drive `act` from `pytest`, capture uploaded artefacts through the artefact
server, and assert on structured logs and files as a black-box workflow test.

## Release artefacts

Each matrix build writes artefacts under `artifacts/<os>-<arch>`. The uploaded
binary uses the package-name-based pattern `dear-diary-<os>-<arch>`.

For Linux GNU `cargo-binstall` targets, the shared staging action also creates:

```plaintext
dear-diary-<version>-<target>.tar.gz
dear-diary-<version>-<target>.tar.gz.sha256
```

The archive contains the `dear-diary` executable at the archive root. Checksum
files contain only the asset basename, so downstream users can run
`sha256sum -c` from the download directory.
