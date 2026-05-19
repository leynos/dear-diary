"""Tests for release workflow helper behaviour."""

from __future__ import annotations

import tarfile
from pathlib import Path

import pytest

from scripts.release_support import (
    ArtifactRequest,
    assert_tag_matches_version,
    load_package_version,
    prepare_artifacts,
    write_sha256_manifest,
)

PROJECT_ROOT = Path(__file__).resolve().parents[2]


def write_manifest_pair(project_root: Path) -> Path:
    """Write workspace and package manifests for release version tests."""
    (project_root / "Cargo.toml").write_text(
        """
[workspace]

[workspace.package]
version = "0.1.0"
""".strip(),
        encoding="utf-8",
    )
    crate_dir = project_root / "crates" / "dear-diary"
    crate_dir.mkdir(parents=True)
    cargo_toml_path = crate_dir / "Cargo.toml"
    cargo_toml_path.write_text(
        """
[package]
name = "dear-diary"
version.workspace = true
""".strip(),
        encoding="utf-8",
    )
    return cargo_toml_path.relative_to(project_root)


def test_load_package_version_resolves_workspace_version(tmp_path: Path) -> None:
    cargo_toml_path = write_manifest_pair(tmp_path)

    version = load_package_version(tmp_path, cargo_toml_path)

    assert version == "0.1.0"


def test_dear_diary_manifest_defines_binstall_metadata() -> None:
    manifest = (PROJECT_ROOT / "crates" / "dear-diary" / "Cargo.toml").read_text(
        encoding="utf-8"
    )

    assert "[package.metadata.binstall]" in manifest
    assert "{ name }-{ version }-{ target }.tar.gz" in manifest
    assert 'bin-dir = "{ bin }{ binary-ext }"' in manifest
    assert 'pkg-fmt = "tgz"' in manifest


def test_assert_tag_matches_version_accepts_prefixed_tag() -> None:
    assert_tag_matches_version("v0.1.0", "0.1.0")


def test_assert_tag_matches_version_rejects_mismatch() -> None:
    with pytest.raises(ValueError, match="does not match"):
        assert_tag_matches_version("v0.2.0", "0.1.0")


def test_write_sha256_manifest_uses_asset_basename(tmp_path: Path) -> None:
    asset = tmp_path / "artifacts" / "linux-x86_64" / "dear-diary-linux-x86_64"
    asset.parent.mkdir(parents=True)
    asset.write_bytes(b"release binary")

    manifest_path = write_sha256_manifest(asset)

    manifest = manifest_path.read_text(encoding="utf-8")
    assert manifest.endswith("  dear-diary-linux-x86_64\n")
    assert "artifacts/" not in manifest


def test_prepare_artifacts_creates_binstall_archive_with_binary_at_root(
    tmp_path: Path,
) -> None:
    binary_path = (
        tmp_path
        / "target"
        / "x86_64-unknown-linux-gnu"
        / "release"
        / "dear-diary"
    )
    binary_path.parent.mkdir(parents=True)
    binary_path.write_bytes(b"release binary")

    outputs = prepare_artifacts(
        ArtifactRequest(
            project_root=tmp_path,
            package_name="dear-diary",
            version="0.1.0",
            target="x86_64-unknown-linux-gnu",
            os_name="linux",
            arch="x86_64",
            ext="",
            cargo_binstall_archive=True,
        )
    )

    output_names = {path.name for path in outputs}
    assert "dear-diary-linux-x86_64" in output_names
    assert "dear-diary-0.1.0-x86_64-unknown-linux-gnu.tar.gz" in output_names

    archive_path = (
        tmp_path
        / "artifacts"
        / "linux-x86_64"
        / "dear-diary-0.1.0-x86_64-unknown-linux-gnu.tar.gz"
    )
    with tarfile.open(archive_path, "r:gz") as archive:
        assert archive.getnames() == ["dear-diary"]

    checksum = archive_path.with_name(f"{archive_path.name}.sha256").read_text(
        encoding="utf-8"
    )
    assert checksum.endswith(
        "  dear-diary-0.1.0-x86_64-unknown-linux-gnu.tar.gz\n"
    )
