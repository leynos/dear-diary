#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["cyclopts>=2.9"]
# ///
"""Release workflow helpers for Dear Diary binary artefacts.

This module contains the testable parts of the GitHub release workflow:
Cargo manifest version resolution, tag validation, release binary copying,
`cargo-binstall` archive creation, and SHA-256 manifest generation. The
workflow calls the Cyclopts commands directly through `uv`, while tests import
the pure helper functions.

Typical usage is from GitHub Actions with `INPUT_*` environment variables.

Examples
--------
Verify that a tag matches the installable crate version::

    INPUT_CARGO_TOML_PATH=crates/dear-diary/Cargo.toml \\
    INPUT_GITHUB_REF_NAME=v0.1.0 \\
    uv run scripts/release_support.py verify-version

Prepare one Linux release artefact set::

    INPUT_PACKAGE_NAME=dear-diary INPUT_VERSION=v0.1.0 \\
    INPUT_TARGET=x86_64-unknown-linux-gnu INPUT_OS=linux \\
    INPUT_ARCH=x86_64 INPUT_CARGO_BINSTALL_ARCHIVE=true \\
    uv run scripts/release_support.py prepare-artifact
"""

from __future__ import annotations

import hashlib
import os
import shutil
import tarfile
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Annotated

import cyclopts
from cyclopts import App, Parameter

app = App(config=cyclopts.config.Env("INPUT_", command=False))


@dataclass(frozen=True)
class ArtifactRequest:
    """Configuration required to prepare one release artefact set.

    Parameters
    ----------
    project_root : pathlib.Path
        Repository root containing `target/` and receiving `artifacts/`.
    package_name : str
        Cargo package and binary name, for example `dear-diary`.
    version : str
        Release version without the leading `v`.
    target : str
        Rust target triple used for the release build.
    os_name : str
        Operating system label used in the uploaded binary name.
    arch : str
        Architecture label used in the uploaded binary name.
    ext : str
        Platform executable suffix, such as `.exe` on Windows or an empty
        string on Unix targets.
    cargo_binstall_archive : bool
        Whether to create a `cargo-binstall` tarball alongside the binary.

    Returns
    -------
    ArtifactRequest
        Immutable request object consumed by :func:`prepare_artifacts`.

    Raises
    ------
    None
        Dataclass construction performs no filesystem validation.

    Notes
    -----
    `prepare_artifacts` performs the filesystem side effects described by this
    request.

    Examples
    --------
    >>> from pathlib import Path
    >>> request = ArtifactRequest(
    ...     project_root=Path("."),
    ...     package_name="dear-diary",
    ...     version="0.1.0",
    ...     target="x86_64-unknown-linux-gnu",
    ...     os_name="linux",
    ...     arch="x86_64",
    ...     ext="",
    ...     cargo_binstall_archive=True,
    ... )
    >>> request.package_name
    'dear-diary'
    """

    project_root: Path
    package_name: str
    version: str
    target: str
    os_name: str
    arch: str
    ext: str
    cargo_binstall_archive: bool


def load_package_version(project_root: Path, cargo_toml_path: Path) -> str:
    """Return the package version, resolving workspace inheritance.

    The installable crate inherits its version from the virtual workspace, so
    release validation has to understand `version.workspace = true`.

    Parameters
    ----------
    project_root : pathlib.Path
        Repository root containing the workspace `Cargo.toml`.
    cargo_toml_path : pathlib.Path
        Manifest path for the installable package, relative to `project_root`.

    Returns
    -------
    str
        Resolved non-empty package version.

    Raises
    ------
    FileNotFoundError
        If either manifest cannot be read.
    tomllib.TOMLDecodeError
        If either manifest is not valid TOML.
    ValueError
        If the package version is missing, malformed, or cannot be resolved
        from the workspace package table.

    Examples
    --------
    >>> from pathlib import Path
    >>> load_package_version(Path("."), Path("crates/dear-diary/Cargo.toml"))
    '0.1.0'
    """
    root_manifest = tomllib.loads(
        (project_root / "Cargo.toml").read_text(encoding="utf-8")
    )
    package_manifest = tomllib.loads(
        (project_root / cargo_toml_path).read_text(encoding="utf-8")
    )
    package = package_manifest.get("package")
    workspace_package = root_manifest.get("workspace", {}).get("package")
    version = package.get("version") if isinstance(package, dict) else None
    if isinstance(version, dict) and version.get("workspace") is True:
        version = (
            workspace_package.get("version")
            if isinstance(workspace_package, dict)
            else None
        )
    if not isinstance(version, str) or not version:
        raise ValueError("invalid or missing package version in manifests")
    return version


def assert_tag_matches_version(tag_name: str, cargo_version: str) -> None:
    """Validate that a release tag matches the Cargo package version.

    Parameters
    ----------
    tag_name : str
        GitHub tag name, with or without a leading `v`.
    cargo_version : str
        Version resolved from Cargo manifests.

    Returns
    -------
    None
        The function returns only when the versions match.

    Raises
    ------
    ValueError
        If the tag version and Cargo version differ.

    Examples
    --------
    >>> assert_tag_matches_version("v0.1.0", "0.1.0")
    >>> assert_tag_matches_version("0.1.0", "0.1.0")
    """
    tag_version = tag_name.removeprefix("v")
    if tag_version != cargo_version:
        msg = (
            f"Tag version {tag_version} does not match Cargo.toml version "
            f"{cargo_version}"
        )
        raise ValueError(msg)


def write_sha256_manifest(path: Path) -> Path:
    """Write a SHA-256 manifest containing only the artefact basename.

    Parameters
    ----------
    path : pathlib.Path
        Artefact file to hash.

    Returns
    -------
    pathlib.Path
        Path to the `.sha256` manifest written next to `path`.

    Raises
    ------
    FileNotFoundError
        If `path` does not exist.
    OSError
        If the artefact cannot be read or the manifest cannot be written.

    Notes
    -----
    This function writes a sibling file as a side effect. The manifest uses the
    artefact basename so users can run `sha256sum -c` from the download
    directory.

    Examples
    --------
    >>> from pathlib import Path
    >>> path = Path("artifacts/linux-x86_64/dear-diary-linux-x86_64")
    >>> manifest = write_sha256_manifest(path)
    >>> manifest.name
    'dear-diary-linux-x86_64.sha256'
    """
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    manifest_path = path.with_name(f"{path.name}.sha256")
    manifest_path.write_text(f"{digest}  {path.name}\n", encoding="utf-8")
    return manifest_path


def prepare_artifacts(request: ArtifactRequest) -> list[Path]:
    """Prepare release binaries, checksums, and optional binstall archives.

    Parameters
    ----------
    request : ArtifactRequest
        Release artefact preparation request.

    Returns
    -------
    list[pathlib.Path]
        Paths to every generated artefact and checksum manifest.

    Raises
    ------
    FileNotFoundError
        If the release binary for `request.target` is missing.
    OSError
        If artefact directories, copied binaries, archives, or checksum
        manifests cannot be created.
    tarfile.TarError
        If the `cargo-binstall` archive cannot be written.

    Notes
    -----
    This function creates files under `artifacts/<os>-<arch>` and may
    overwrite existing artefacts with the same names.

    Examples
    --------
    >>> from pathlib import Path
    >>> request = ArtifactRequest(
    ...     project_root=Path("."),
    ...     package_name="dear-diary",
    ...     version="0.1.0",
    ...     target="x86_64-unknown-linux-gnu",
    ...     os_name="linux",
    ...     arch="x86_64",
    ...     ext="",
    ...     cargo_binstall_archive=True,
    ... )
    >>> outputs = prepare_artifacts(request)  # doctest: +SKIP
    >>> len(outputs)  # doctest: +SKIP
    4
    """
    artifact_dir = request.project_root / "artifacts" / (
        f"{request.os_name}-{request.arch}"
    )
    binary_path = (
        request.project_root
        / "target"
        / request.target
        / "release"
        / f"{request.package_name}{request.ext}"
    )
    binary_name = f"{request.package_name}-{request.os_name}-{request.arch}{request.ext}"

    artifact_dir.mkdir(parents=True, exist_ok=True)
    copied_binary = artifact_dir / binary_name
    shutil.copy2(binary_path, copied_binary)

    outputs = [copied_binary, write_sha256_manifest(copied_binary)]
    if request.cargo_binstall_archive:
        archive_path = artifact_dir / (
            f"{request.package_name}-{request.version}-{request.target}.tar.gz"
        )
        with tarfile.open(archive_path, "w:gz") as archive:
            archive.add(binary_path, arcname=f"{request.package_name}{request.ext}")
        outputs.extend([archive_path, write_sha256_manifest(archive_path)])
    return outputs


@app.command
def verify_version(
    *,
    cargo_toml_path: Annotated[Path, Parameter(required=True)],
    github_ref_name: Annotated[str, Parameter(required=True)],
    project_root: Path = Path("."),
) -> None:
    """Verify that the release tag matches the Cargo package version.

    Parameters
    ----------
    cargo_toml_path : pathlib.Path
        Manifest path for the installable package, relative to `project_root`.
    github_ref_name : str
        GitHub reference name for the release tag.
    project_root : pathlib.Path, optional
        Repository root. Defaults to the current working directory.

    Returns
    -------
    None
        Prints a success message when the tag matches.

    Raises
    ------
    FileNotFoundError
        If a required Cargo manifest cannot be read.
    tomllib.TOMLDecodeError
        If a Cargo manifest is invalid TOML.
    ValueError
        If the manifest version is invalid or the tag does not match it.

    Notes
    -----
    This command writes to standard output and is used by the GitHub release
    workflow before any build artefacts are published.

    Examples
    --------
    >>> verify_version(
    ...     cargo_toml_path=Path("crates/dear-diary/Cargo.toml"),
    ...     github_ref_name="v0.1.0",
    ... )  # doctest: +SKIP
    """
    root = project_root.resolve()
    cargo_version = load_package_version(root, cargo_toml_path)
    assert_tag_matches_version(github_ref_name, cargo_version)
    print(f"Release tag {github_ref_name.removeprefix('v')} matches Cargo.toml.")


@app.command
def prepare_artifact(
    *,
    package_name: Annotated[str, Parameter(required=True)],
    version: Annotated[str, Parameter(required=True)],
    target: Annotated[str, Parameter(required=True)],
    os_name: Annotated[str, Parameter(required=True, env_var="INPUT_OS")],
    arch: Annotated[str, Parameter(required=True)],
    ext: str = "",
    cargo_binstall_archive: bool = False,
    project_root: Path = Path("."),
) -> None:
    """Prepare one release artefact set for the workflow matrix entry.

    Parameters
    ----------
    package_name : str
        Cargo package and binary name.
    version : str
        GitHub release tag or version. A leading `v` is stripped before archive
        names are composed.
    target : str
        Rust target triple for the compiled binary.
    os_name : str
        Operating system label used in the uploaded binary name.
    arch : str
        Architecture label used in the uploaded binary name.
    ext : str, optional
        Platform executable suffix. Defaults to an empty string.
    cargo_binstall_archive : bool, optional
        Whether to create a `cargo-binstall` archive. Defaults to `False`.
    project_root : pathlib.Path, optional
        Repository root. Defaults to the current working directory.

    Returns
    -------
    None
        Prints generated artefact paths for workflow diagnostics.

    Raises
    ------
    FileNotFoundError
        If the expected release binary is missing.
    OSError
        If artefact output files cannot be created.
    tarfile.TarError
        If archive creation fails.

    Notes
    -----
    This command writes release artefacts under `artifacts/<os>-<arch>` and is
    intentionally idempotent for reruns that prepare the same matrix target.

    Examples
    --------
    >>> prepare_artifact(
    ...     package_name="dear-diary",
    ...     version="v0.1.0",
    ...     target="x86_64-unknown-linux-gnu",
    ...     os_name="linux",
    ...     arch="x86_64",
    ...     cargo_binstall_archive=True,
    ... )  # doctest: +SKIP
    """
    release_version = version.removeprefix("v")
    outputs = prepare_artifacts(
        ArtifactRequest(
            project_root=project_root.resolve(),
            package_name=package_name,
            version=release_version,
            target=target,
            os_name=os_name,
            arch=arch,
            ext=ext,
            cargo_binstall_archive=cargo_binstall_archive,
        )
    )
    print(f"Prepared {len(outputs)} release artefacts:")
    for output in outputs:
        print(f"- {output.relative_to(project_root.resolve())}")


if __name__ == "__main__":
    app()
