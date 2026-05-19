#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["cyclopts>=2.9"]
# ///
"""Release workflow helpers for Dear Diary binary artefacts."""

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
    """Configuration required to prepare one release artefact set."""

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
    """
    root_manifest = tomllib.loads(
        (project_root / "Cargo.toml").read_text(encoding="utf-8")
    )
    package_manifest = tomllib.loads(
        (project_root / cargo_toml_path).read_text(encoding="utf-8")
    )
    version = package_manifest["package"].get("version")
    if isinstance(version, dict) and version.get("workspace"):
        version = root_manifest["workspace"]["package"]["version"]
    if not isinstance(version, str) or not version:
        msg = (
            f"Could not read package.version from {cargo_toml_path} or "
            "workspace.package.version from Cargo.toml."
        )
        raise ValueError(msg)
    return version


def assert_tag_matches_version(tag_name: str, cargo_version: str) -> None:
    """Validate that a release tag matches the Cargo package version."""
    tag_version = tag_name.removeprefix("v")
    if tag_version != cargo_version:
        msg = (
            f"Tag version {tag_version} does not match Cargo.toml version "
            f"{cargo_version}"
        )
        raise ValueError(msg)


def write_sha256_manifest(path: Path) -> Path:
    """Write a SHA-256 manifest containing only the artefact basename."""
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    manifest_path = path.with_name(f"{path.name}.sha256")
    manifest_path.write_text(f"{digest}  {path.name}\n", encoding="utf-8")
    return manifest_path


def prepare_artifacts(request: ArtifactRequest) -> list[Path]:
    """Prepare release binaries, checksums, and optional binstall archives."""
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
    """Verify that the release tag matches the Cargo package version."""
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
    """Prepare one release artefact set for the workflow matrix entry."""
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
