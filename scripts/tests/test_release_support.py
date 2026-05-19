"""Tests for release workflow helper behaviour.

This module verifies the release automation surfaces used by GitHub Actions:
`release_support` command wiring, `release_version` manifest and tag checks,
and `release_packaging` binary, checksum, and `cargo-binstall` artefact
creation. The tests cover direct helper functions, command-style entry points,
environment-variable execution through Cyclopts, and workflow upload-script
contracts so changes to release automation are caught before a tag publish.
"""

from __future__ import annotations

import os
import subprocess
import sys
import tarfile
import tomllib
from pathlib import Path

import pytest
from syrupy.assertion import SnapshotAssertion

SCRIPTS_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import release_support
from scripts.release_packaging import (
    ArtifactRequest,
    prepare_artifacts,
    write_sha256_manifest,
)
from scripts.release_version import assert_tag_matches_version, load_package_version

PROJECT_ROOT = Path(__file__).resolve().parents[2]
RELEASE_SCRIPT = PROJECT_ROOT / "scripts" / "release_support.py"
RELEASE_WORKFLOW = PROJECT_ROOT / ".github" / "workflows" / "release.yml"


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
    """Resolve a workspace-inherited package version from test manifests."""
    cargo_toml_path = write_manifest_pair(tmp_path)

    version = load_package_version(tmp_path, cargo_toml_path)

    assert version == "0.1.0"


def test_load_package_version_reports_consistent_manifest_error(
    tmp_path: Path,
) -> None:
    """Report one consistent error when workspace version lookup fails."""
    (tmp_path / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
    crate_dir = tmp_path / "crates" / "dear-diary"
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

    with pytest.raises(ValueError, match="invalid or missing package version"):
        load_package_version(tmp_path, cargo_toml_path.relative_to(tmp_path))


def test_dear_diary_manifest_defines_binstall_metadata() -> None:
    """Parse the installable crate manifest and assert binstall metadata."""
    manifest = (PROJECT_ROOT / "crates" / "dear-diary" / "Cargo.toml").read_text(
        encoding="utf-8"
    )
    parsed_manifest = tomllib.loads(manifest)
    binstall_metadata = parsed_manifest["package"]["metadata"]["binstall"]
    linux_overrides = binstall_metadata["overrides"][
        'cfg(all(target_os = "linux", any(target_arch = "x86_64", '
        'target_arch = "aarch64"), target_env = "gnu"))'
    ]

    assert linux_overrides["pkg-url"] == (
        "{ repo }/releases/download/v{ version }/"
        "{ name }-{ version }-{ target }.tar.gz"
    )
    assert linux_overrides["bin-dir"] == "{ bin }{ binary-ext }"
    assert linux_overrides["pkg-fmt"] == "tgz"


def test_assert_tag_matches_version_accepts_prefixed_tag() -> None:
    """Accept GitHub release tags whose version matches Cargo metadata."""
    assert_tag_matches_version("v0.1.0", "0.1.0")


def test_assert_tag_matches_version_accepts_plain_tag() -> None:
    """Accept release tags that omit the conventional leading v."""
    assert_tag_matches_version("0.1.0", "0.1.0")


def test_assert_tag_matches_version_rejects_mismatch() -> None:
    """Reject GitHub release tags whose version differs from Cargo metadata."""
    with pytest.raises(ValueError, match="does not match"):
        assert_tag_matches_version("v0.2.0", "0.1.0")


def test_write_sha256_manifest_uses_asset_basename(tmp_path: Path) -> None:
    """Write checksum manifests that contain only the asset basename."""
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
    """Create binstall archives with the binary at the archive root."""
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


def test_prepare_artifacts_skips_binstall_archive_when_disabled(
    tmp_path: Path,
) -> None:
    """Create only binary and checksum outputs when binstall is disabled."""
    binary_path = (
        tmp_path
        / "target"
        / "x86_64-unknown-freebsd"
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
            target="x86_64-unknown-freebsd",
            os_name="freebsd",
            arch="x86_64",
            ext="",
            cargo_binstall_archive=False,
        )
    )

    output_names = {path.name for path in outputs}
    assert output_names == {
        "dear-diary-freebsd-x86_64",
        "dear-diary-freebsd-x86_64.sha256",
    }


def test_prepare_artifacts_reports_missing_binary(tmp_path: Path) -> None:
    """Raise a file-not-found error when the expected binary is absent."""
    request = ArtifactRequest(
        project_root=tmp_path,
        package_name="dear-diary",
        version="0.1.0",
        target="x86_64-unknown-linux-gnu",
        os_name="linux",
        arch="x86_64",
        ext="",
        cargo_binstall_archive=True,
    )

    with pytest.raises(FileNotFoundError):
        prepare_artifacts(request)


def test_verify_version_command_prints_success(
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Exercise the verify-version Cyclopts command body."""
    cargo_toml_path = write_manifest_pair(tmp_path)

    release_support.verify_version(
        cargo_toml_path=cargo_toml_path,
        github_ref_name="v0.1.0",
        project_root=tmp_path,
    )

    assert "Release tag 0.1.0 matches Cargo.toml." in capsys.readouterr().out


def test_prepare_artifact_command_prints_outputs(
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Exercise the prepare-artifact Cyclopts command body."""
    binary_path = (
        tmp_path
        / "target"
        / "x86_64-unknown-linux-gnu"
        / "release"
        / "dear-diary"
    )
    binary_path.parent.mkdir(parents=True)
    binary_path.write_bytes(b"release binary")

    release_support.prepare_artifact(
        package_name="dear-diary",
        version="v0.1.0",
        target="x86_64-unknown-linux-gnu",
        os_name="linux",
        arch="x86_64",
        cargo_binstall_archive=True,
        project_root=tmp_path,
    )

    output = capsys.readouterr().out
    assert "Prepared 4 release artefacts:" in output
    assert "dear-diary-0.1.0-x86_64-unknown-linux-gnu.tar.gz" in output


def test_prepare_artifact_command_logs_missing_binary(
    tmp_path: Path,
    caplog: pytest.LogCaptureFixture,
) -> None:
    """Log command context when artefact preparation fails."""
    caplog.set_level("INFO")

    with pytest.raises(FileNotFoundError):
        release_support.prepare_artifact(
            package_name="dear-diary",
            version="v0.1.0",
            target="x86_64-unknown-linux-gnu",
            os_name="linux",
            arch="x86_64",
            cargo_binstall_archive=True,
            project_root=tmp_path,
        )

    assert "operation=prepare_artifact phase=failed" in caplog.text
    assert "target=x86_64-unknown-linux-gnu" in caplog.text


def test_prepare_artifacts_supports_aarch64_binstall_target(
    tmp_path: Path,
) -> None:
    """Create the expected archive name for Linux aarch64 releases."""
    binary_path = (
        tmp_path
        / "target"
        / "aarch64-unknown-linux-gnu"
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
            target="aarch64-unknown-linux-gnu",
            os_name="linux",
            arch="aarch64",
            ext="",
            cargo_binstall_archive=True,
        )
    )

    output_names = {path.name for path in outputs}
    assert "dear-diary-linux-aarch64" in output_names
    assert "dear-diary-0.1.0-aarch64-unknown-linux-gnu.tar.gz" in output_names


def test_verify_version_command_resolves_environment_variables(
    tmp_path: Path,
) -> None:
    """Exercise Cyclopts environment variable mapping for verify-version."""
    cargo_toml_path = write_manifest_pair(tmp_path)
    env = os.environ | {
        "INPUT_CARGO_TOML_PATH": str(cargo_toml_path),
        "INPUT_GITHUB_REF_NAME": "v0.1.0",
    }

    result = subprocess.run(
        [sys.executable, str(RELEASE_SCRIPT), "verify-version", "--project-root", str(tmp_path)],
        check=True,
        capture_output=True,
        env=env,
        text=True,
    )

    assert "Release tag 0.1.0 matches Cargo.toml." in result.stdout


def test_release_upload_script_uses_bounded_metric_labels() -> None:
    """Assert GitHub release upload metrics do not include unbounded paths."""
    workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")

    assert 'echo "Uploading ${file}"' in workflow
    assert 'gh release upload "${{ github.ref_name }}" "$file" --clobber' in workflow
    assert 'echo "metric=release_asset_uploaded count=${uploaded_count}"' in workflow
    assert "metric=release_asset_uploaded count=${uploaded_count} file=" not in workflow


def test_verify_version_command_output_snapshot(
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    snapshot: SnapshotAssertion,
) -> None:
    """Snapshot the verify-version command output structure."""
    cargo_toml_path = write_manifest_pair(tmp_path)

    release_support.verify_version(
        cargo_toml_path=cargo_toml_path,
        github_ref_name="v0.1.0",
        project_root=tmp_path,
    )

    assert capsys.readouterr().out == snapshot


def test_prepare_artifact_command_output_snapshot(
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    snapshot: SnapshotAssertion,
) -> None:
    """Snapshot the prepare-artifact command output structure."""
    binary_path = (
        tmp_path
        / "target"
        / "x86_64-unknown-linux-gnu"
        / "release"
        / "dear-diary"
    )
    binary_path.parent.mkdir(parents=True)
    binary_path.write_bytes(b"release binary")

    release_support.prepare_artifact(
        package_name="dear-diary",
        version="v0.1.0",
        target="x86_64-unknown-linux-gnu",
        os_name="linux",
        arch="x86_64",
        cargo_binstall_archive=True,
        project_root=tmp_path,
    )

    assert capsys.readouterr().out == snapshot
