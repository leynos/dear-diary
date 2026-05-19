#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["cyclopts>=2.9"]
# ///
"""Command-line interface for Dear Diary release helpers.

This module keeps GitHub Actions wiring thin. Version parsing lives in
`release_version`, and artefact packaging lives in `release_packaging`; this
file only maps `INPUT_*` environment variables to those helpers.

Typical usage is from GitHub Actions with `uv`.

Examples
--------
Verify the release tag against the installable package manifest::

    INPUT_CARGO_TOML_PATH=crates/dear-diary/Cargo.toml \\
    INPUT_GITHUB_REF_NAME=v0.1.0 \\
    uv run scripts/release_support.py verify-version

Prepare one release artefact set::

    INPUT_PACKAGE_NAME=dear-diary INPUT_VERSION=v0.1.0 \\
    INPUT_TARGET=x86_64-unknown-linux-gnu INPUT_OS=linux \\
    INPUT_ARCH=x86_64 INPUT_CARGO_BINSTALL_ARCHIVE=true \\
    uv run scripts/release_support.py prepare-artifact
"""

from __future__ import annotations

import logging
from pathlib import Path
from typing import Annotated

import cyclopts
from cyclopts import App, Parameter

from release_packaging import ArtifactRequest, prepare_artifacts
from release_version import assert_tag_matches_version, load_package_version

logging.basicConfig(level=logging.INFO, format="%(levelname)s %(name)s: %(message)s")
logger = logging.getLogger("release_support")

app = App(config=cyclopts.config.Env("INPUT_", command=False))


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
    logger.info(
        "operation=verify_version phase=start project_root=%s cargo_toml_path=%s tag=%s",
        root,
        cargo_toml_path,
        github_ref_name,
    )
    try:
        cargo_version = load_package_version(root, cargo_toml_path)
        assert_tag_matches_version(github_ref_name, cargo_version)
    except Exception:
        logger.exception(
            "operation=verify_version phase=failed project_root=%s cargo_toml_path=%s tag=%s",
            root,
            cargo_toml_path,
            github_ref_name,
        )
        raise
    logger.info(
        "operation=verify_version phase=complete tag=%s cargo_version=%s",
        github_ref_name,
        cargo_version,
    )
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
    root = project_root.resolve()
    release_version = version.removeprefix("v")
    logger.info(
        (
            "operation=prepare_artifact phase=start project_root=%s package=%s "
            "version=%s target=%s os=%s arch=%s binstall=%s"
        ),
        root,
        package_name,
        release_version,
        target,
        os_name,
        arch,
        cargo_binstall_archive,
    )
    try:
        outputs = prepare_artifacts(
            ArtifactRequest(
                project_root=root,
                package_name=package_name,
                version=release_version,
                target=target,
                os_name=os_name,
                arch=arch,
                ext=ext,
                cargo_binstall_archive=cargo_binstall_archive,
            )
        )
    except Exception:
        logger.exception(
            (
                "operation=prepare_artifact phase=failed project_root=%s package=%s "
                "version=%s target=%s os=%s arch=%s binstall=%s"
            ),
            root,
            package_name,
            release_version,
            target,
            os_name,
            arch,
            cargo_binstall_archive,
        )
        raise
    logger.info(
        "operation=prepare_artifact phase=complete metric=release_artifacts_prepared count=%d",
        len(outputs),
    )
    print(f"Prepared {len(outputs)} release artefacts:")
    for output in outputs:
        print(f"- {output.relative_to(root)}")


if __name__ == "__main__":
    app()
