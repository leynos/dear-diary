"""Cargo manifest version helpers for release validation.

This module resolves the installable package version from Cargo manifests and
validates that GitHub release tags match it. It deliberately contains no
GitHub Actions or packaging code, so manifest parsing can be tested directly.

Examples
--------
Resolve the current package version::

    from pathlib import Path
    version = load_package_version(Path("."), Path("crates/dear-diary/Cargo.toml"))

Validate a tag before publishing::

    assert_tag_matches_version("v0.1.0", version)
"""

from __future__ import annotations

import logging
import tomllib
from pathlib import Path

logger = logging.getLogger("release_support.version")


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
    logger.info(
        "operation=load_package_version phase=start project_root=%s cargo_toml_path=%s",
        project_root,
        cargo_toml_path,
    )
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
        logger.error(
            "operation=load_package_version phase=invalid project_root=%s cargo_toml_path=%s",
            project_root,
            cargo_toml_path,
        )
        raise ValueError("invalid or missing package version in manifests")
    logger.info(
        "operation=load_package_version phase=complete cargo_version=%s",
        version,
    )
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
    logger.info(
        "operation=assert_tag_matches_version phase=start tag=%s cargo_version=%s",
        tag_name,
        cargo_version,
    )
    tag_version = tag_name.removeprefix("v")
    if tag_version != cargo_version:
        msg = (
            f"Tag version {tag_version} does not match Cargo.toml version "
            f"{cargo_version}"
        )
        logger.error(
            "operation=assert_tag_matches_version phase=mismatch tag_version=%s cargo_version=%s",
            tag_version,
            cargo_version,
        )
        raise ValueError(msg)
    logger.info(
        "operation=assert_tag_matches_version phase=complete tag_version=%s",
        tag_version,
    )
