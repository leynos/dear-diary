#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["cyclopts>=2.9"]
# ///
"""Command-line interface for Dear Diary release helpers.

This module keeps GitHub Actions wiring thin. Version parsing lives in
`release_version`; this file only maps `INPUT_*` environment variables to that
helper.

Typical usage is from GitHub Actions with `uv`.

Examples
--------
Verify the release tag against the installable package manifest::

    INPUT_CARGO_TOML_PATH=crates/dear-diary/Cargo.toml \\
    INPUT_GITHUB_REF_NAME=v0.1.0 \\
    uv run scripts/release_support.py verify-version

"""

from __future__ import annotations

import logging
from pathlib import Path
from typing import Annotated

import cyclopts
from cyclopts import App, Parameter

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


if __name__ == "__main__":
    app()
