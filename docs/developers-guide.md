# Developer guide

This guide covers release automation and local validation for contributors.

## Build configuration

The workspace uses the pinned Nightly toolchain declared in
`rust-toolchain.toml`. Nightly is required for the Rust 2024 edition and for
Cargo's unstable `codegen-backend` profile configuration.

Development builds use the Cranelift code generation backend through
`.cargo/config.toml`:

```toml
[unstable]
codegen-backend = true

[profile.dev]
codegen-backend = "cranelift"
```

The pinned toolchain must include these Rust components:

- `rustfmt`
- `clippy`
- `rustc-codegen-cranelift`

Install or repair the pinned toolchain using `rust-toolchain.toml` as the
channel source of truth:

```bash
rustup toolchain install
rustup component add rustc-codegen-cranelift
```

Linux `x86_64-unknown-linux-gnu` builds link through `clang` with `mold`:

```toml
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]
```

Install `clang` and `mold` before running local Linux builds that use that host
target. In GitHub Actions, the CI and release workflows install both packages
on Linux runners before invoking Cargo.

Coverage generation is the intentional exception to Cranelift. `cargo-llvm-cov`
requires LLVM coverage instrumentation, so CI uses the shared
`generate-coverage` action revision that carries the LLVM backend carve-out for
coverage runs. Do not add a step-level
`CARGO_PROFILE_DEV_CODEGEN_BACKEND=llvm` override: that environment also
affects the action's internal tool installation and can make Cargo reject the
unstable profile setting before coverage starts.

Any change to `.cargo/config.toml`, `rust-toolchain.toml`, or the build-related
GitHub Actions wiring must include script-test coverage that verifies the
configuration contract. At minimum, tests should cover the selected codegen
backend, the Cranelift component, the Linux linker settings, guarded CI
installation of `clang` and `mold`, and the coverage action carve-out.

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

## Release helper script

Release version checks live in `scripts/release_version.py`. Artefact copying,
archive generation, and checksum creation live in
`scripts/release_packaging.py`. The `scripts/release_support.py` script keeps
the command-line wiring thin. Keep this logic in scripts rather than embedding
it directly in workflow shell blocks so it remains testable.

The helper is split across focused modules to keep each file comfortably under
the project size limit and to make command wiring, manifest validation, and
packaging side effects independently reviewable. Release automation also pins
external Git references rather than using floating tags; for example, the
workflow installs `cross` from a fixed revision so reruns use the same source.

The script follows the repository scripting standards:

- `uv` executes the script with its declared Python dependencies.
- Cyclopts maps `INPUT_*` workflow environment variables to typed parameters.
- Pure helper functions in focused modules handle version resolution, tag
  validation, archive creation, and checksum manifest creation.

Run the script tests with:

```bash
make test-scripts
```

The tests verify workspace-inherited Cargo versions, tag/version matching,
Cyclopts environment mapping, GitHub Actions upload wiring, `cargo-binstall`
metadata, archive layout, and checksum manifests that contain only asset
basenames. Snapshot tests pin the command output shape for the release helper
commands.

GitHub Actions installs `uv` with `astral-sh/setup-uv` before running the
release helper. The helper targets Python 3.13, matching the repository
scripting standards for new automation.

## Release artefacts

Each matrix build writes artefacts under `artifacts/<os>-<arch>`. The uploaded
binary uses the package-name-based pattern `dear-diary-<os>-<arch>`.

For Linux GNU `cargo-binstall` targets, the helper also creates:

```plaintext
dear-diary-<version>-<target>.tar.gz
dear-diary-<version>-<target>.tar.gz.sha256
```

The archive contains the `dear-diary` executable at the archive root. Checksum
files contain only the asset basename, so downstream users can run
`sha256sum -c` from the download directory.
