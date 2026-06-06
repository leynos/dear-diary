"""Tests for release workflow helper behaviour.

This module verifies the release automation surfaces used by GitHub Actions:
`release_support` command wiring, `release_version` manifest and tag checks,
and workflow upload-script contracts so changes to release automation are
caught before a tag publish.
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

import pytest
from syrupy.assertion import SnapshotAssertion

SCRIPTS_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import release_support
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
