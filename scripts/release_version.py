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

import tomllib
from pathlib import Path


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
