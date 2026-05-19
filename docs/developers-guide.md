# Developer guide

This guide covers release automation and local validation for contributors.

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
archive generation, and checksum creation live in `scripts/release_packaging.py`.
The `scripts/release_support.py` script keeps the command-line wiring thin.
Keep this logic in scripts rather than embedding it directly in workflow shell
blocks so it remains testable.

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
`cargo-binstall` metadata, archive layout, and checksum manifests that contain
only asset basenames.

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
