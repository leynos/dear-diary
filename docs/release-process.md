# Release process

This project publishes prebuilt binaries for multiple operating systems and
architectures. It also publishes `cargo-binstall` archives for the supported
Linux release targets.

The project uses the Rust toolchain pinned in `rust-toolchain.toml`.

The GitHub Actions workflow `.github/workflows/release.yml` builds and uploads
binaries for:

- Linux (x86_64 and aarch64)
- FreeBSD (x86_64)

Releases start from tags named `v<major>.<minor>.<patch>`. The workflow checks
that the tag's version, without the leading `v`, matches the workspace
`Cargo.toml` version field and aborts if they differ.

Each binary is named using the pattern `dear-diary-<os>-<arch>`.

For Linux `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`, the
workflow also produces `cargo-binstall` archives named
`dear-diary-<version>-<target>.tar.gz`. Each archive contains the `dear-diary`
binary at the archive root, matching the `crates/dear-diary/Cargo.toml`
`[package.metadata.binstall]` configuration.

Binaries are uploaded as soon as they are built, so they are available from the
workflow run while other targets build.

## Workflow details

The `release.yml` workflow defines a matrix of operating system and
architecture combinations. Each entry includes the target triple used by
`cross`. During the build job, `cross` compiles the `dear-diary` package
release binary for every matrix row.

`cross` is installed from a specific git tag to avoid unexpected behaviour from
its main branch. Release staging is delegated to the pinned shared action
`leynos/shared-actions/.github/actions/stage-release-artefacts`, using
`.github/release-staging.toml` as the source of truth for target names,
destination filenames, checksum sidecars, and cargo-binstall archive settings.

Each binary is placed in an `artifacts/<os>-<arch>` directory using the naming
pattern `dear-diary-<os>-<arch>`. An SHA-256 checksum is written alongside each
binary for download verification. The Linux `cargo-binstall` targets
additionally produce `dear-diary-<version>-<target>.tar.gz` plus a matching
SHA-256 checksum. FreeBSD builds publish the binary and checksum only because
the package's cargo-binstall metadata advertises Linux GNU archives.

After every build completes, the artefact is uploaded so that the GitHub
Actions interface provides it immediately. Once the matrix has finished, the
`release` job downloads all artefacts and uploads them to the GitHub release
using `gh release upload`.

## Local workflow validation

Run the black-box workflow harness with:

```bash
make test-workflow
```

This target uses `act` through pytest. It parses the release workflow graph,
runs `.github/workflows/selftest-staging.yml` against a dummy Linux binary, and
asserts that the shared staging action produces the expected binary, checksum,
cargo-binstall archive, and structured metric output.
