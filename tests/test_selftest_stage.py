"""Black-box validation for the release staging selftest workflow.

This module runs `.github/workflows/selftest-staging.yml` through the local
`act` runner, extracts the uploaded `selftest-stage` artefact, and checks the
observable release-staging contract. It verifies the renamed release binary,
checksum sidecars, cargo-binstall archive layout, and the structured metric
line emitted by the workflow.

Examples
--------
Run this validation with the project Makefile::

    make test-workflow

Failures usually mean that the shared staging action, the local staging
configuration, `act` artefact export, or structured workflow logging has
drifted from the release contract.
"""

from __future__ import annotations

import json
import re
import stat
import subprocess
import tarfile
import tomllib
import zipfile
from pathlib import Path
from typing import cast

import pytest

from _act_support import PROJECT_ROOT, ActRunner, combined_logs

SELFTEST_EVENT = Path("tests/fixtures/selftest.event.json")
SELFTEST_WORKFLOW = Path(".github/workflows/selftest-staging.yml")


class ReleaseManifestError(Exception):
    """Report an invalid release manifest shape for workflow tests."""

    @classmethod
    def missing_manifest(cls, manifest_path: Path) -> ReleaseManifestError:
        """Create an error for an unreadable Cargo manifest."""
        return cls(f"missing or unreadable Cargo.toml at {manifest_path}")

    @classmethod
    def missing_version_key(cls, manifest_path: Path) -> ReleaseManifestError:
        """Create an error for a Cargo manifest without a workspace version."""
        return cls(f"missing workspace.package.version in {manifest_path}")

    @classmethod
    def invalid_toml(cls, manifest_path: Path) -> ReleaseManifestError:
        """Create an error for invalid Cargo manifest TOML."""
        return cls(f"invalid TOML in {manifest_path}")

    @classmethod
    def invalid_version_value(cls, manifest_path: Path) -> ReleaseManifestError:
        """Create an error for a non-string workspace version."""
        return cls(f"workspace.package.version is not a string in {manifest_path}")


class ZipSlipError(Exception):
    """Report a zip archive member that escapes its extraction directory."""

    @classmethod
    def escaped_member(cls, member_name: str) -> ZipSlipError:
        """Create an error for an unsafe zip member path."""
        return cls(f"zip member escapes artefact directory: {member_name}")


def _workspace_version() -> str:
    """Return the workspace package version used by release artefact names."""
    manifest_path = PROJECT_ROOT / "Cargo.toml"
    try:
        manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
        raw_version = cast("object", manifest["workspace"]["package"]["version"])
    except OSError as error:
        raise ReleaseManifestError.missing_manifest(manifest_path) from error
    except KeyError as error:
        raise ReleaseManifestError.missing_version_key(manifest_path) from error
    except tomllib.TOMLDecodeError as error:
        raise ReleaseManifestError.invalid_toml(manifest_path) from error
    match raw_version:
        case str():
            return raw_version
        case _:
            raise ReleaseManifestError.invalid_version_value(manifest_path)


def _binstall_archive_name() -> str:
    """Return the cargo-binstall archive filename expected for this workspace."""
    return f"dear-diary-{_workspace_version()}-x86_64-unknown-linux-gnu.tar.gz"


def _run_stage_only(act_runner: ActRunner) -> subprocess.CompletedProcess[str]:
    """Run the selftest staging job and assert that `act` succeeded."""
    result = act_runner(
        "pull_request",
        job="stage-only",
        event_path=SELFTEST_EVENT,
        workflow=SELFTEST_WORKFLOW,
    )
    assert result.returncode == 0, combined_logs(result)
    return result


def _artifact_root(act_runner: ActRunner) -> Path:
    """Return the directory containing the uploaded selftest artefact."""
    artifact_dir = act_runner.artifact_dir
    archive_path = next(artifact_dir.rglob("selftest-stage.zip"), None)
    assert archive_path is not None, f"missing uploaded artefact under {artifact_dir}"
    extract_dir = artifact_dir / "selftest-stage"
    with zipfile.ZipFile(archive_path) as archive:
        _extract_safe_zip(archive, extract_dir)
    return extract_dir


def _extract_safe_zip(archive: zipfile.ZipFile, extract_dir: Path) -> None:
    """Extract a zip archive after rejecting entries outside `extract_dir`."""
    resolved_extract_dir = extract_dir.resolve()
    for member in archive.infolist():
        mode = member.external_attr >> 16
        if stat.S_ISLNK(mode):
            raise ZipSlipError.escaped_member(member.filename)
        destination = (extract_dir / member.filename).resolve()
        if not destination.is_relative_to(resolved_extract_dir):
            raise ZipSlipError.escaped_member(member.filename)
        archive.extract(member, extract_dir)


@pytest.fixture(scope="module")
def staged_selftest(
    act_runner: ActRunner,
) -> tuple[subprocess.CompletedProcess[str], Path]:
    """Run the staging workflow once and return logs plus extracted artefacts."""
    result = _run_stage_only(act_runner)
    return result, _artifact_root(act_runner)


def test_stage_only_produces_expected_artefacts(
    staged_selftest: tuple[subprocess.CompletedProcess[str], Path],
) -> None:
    """Assert that the selftest workflow exports the staged release files."""
    _result, root = staged_selftest
    binstall_archive_name = _binstall_archive_name()
    expected_files = {
        "dear-diary-linux-x86_64",
        "dear-diary-linux-x86_64.sha256",
        binstall_archive_name,
        f"{binstall_archive_name}.sha256",
    }
    staged_paths = {
        path.relative_to(root).as_posix(): path
        for path in root.rglob("*")
        if path.is_file()
    }
    staged_names = {path.name for path in staged_paths.values()}
    assert expected_files <= staged_names, (
        f"missing staged files: expected={sorted(expected_files)} "
        f"actual={sorted(staged_names)}"
    )
    binary_sidecar = _staged_path_named(staged_paths, "dear-diary-linux-x86_64.sha256")
    binstall_sidecar = _staged_path_named(
        staged_paths,
        f"{binstall_archive_name}.sha256",
    )
    _assert_checksum_sidecar(binary_sidecar, "dear-diary-linux-x86_64")
    _assert_checksum_sidecar(binstall_sidecar, binstall_archive_name)

    archive_path = next(
        (path for path in staged_paths.values() if path.name == binstall_archive_name),
        None,
    )
    assert archive_path is not None, f"missing binstall archive: {staged_paths}"
    with tarfile.open(archive_path, "r:gz") as archive:
        members = archive.getnames()
        assert members == ["dear-diary"], f"unexpected archive members: {members}"


def _staged_path_named(staged_paths: dict[str, Path], filename: str) -> Path:
    """Return a staged path by basename."""
    path = next((path for path in staged_paths.values() if path.name == filename), None)
    assert path is not None, f"missing staged file {filename}: {staged_paths}"
    return path


def _assert_checksum_sidecar(sidecar_path: Path, asset_name: str) -> None:
    """Assert that a checksum sidecar uses the pinned shared-action format."""
    content = sidecar_path.read_text(encoding="utf-8")
    expected = rf"[0-9a-f]{{64}}  {re.escape(asset_name)}\n"
    assert re.fullmatch(expected, content), (
        f"unexpected checksum sidecar format in {sidecar_path}: {content!r}"
    )


def test_stage_only_emits_metric(
    staged_selftest: tuple[subprocess.CompletedProcess[str], Path],
) -> None:
    """Assert that the workflow emits the expected structured metric line."""
    result, _root = staged_selftest

    assert any(
        "metric=selftest_stage_complete target=linux-x86_64"
        in _workflow_event_output(line)
        for line in combined_logs(result).splitlines()
    ), combined_logs(result)


def _workflow_event_output(line: str) -> str:
    """Return the observable message field from one JSON workflow log line."""
    if not line.lstrip().startswith("{"):
        return ""
    try:
        event = json.loads(line)
    except json.JSONDecodeError:
        return ""
    if not isinstance(event, dict):
        return ""
    output = event.get("Output") or event.get("message") or event.get("msg")
    return "" if output is None else str(output)
