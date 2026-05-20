"""Tests for repository build and coverage configuration.

These tests cover infrastructure files that affect Cargo code generation,
linking, and CI coverage behaviour. They keep the Cranelift and mold setup
from drifting silently because these files are not exercised by Rust unit
tests directly.
"""

from __future__ import annotations

import tomllib
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[2]
CARGO_CONFIG = PROJECT_ROOT / ".cargo" / "config.toml"
CI_WORKFLOW = PROJECT_ROOT / ".github" / "workflows" / "ci.yml"
DEVELOPERS_GUIDE = PROJECT_ROOT / "docs" / "developers-guide.md"
README = PROJECT_ROOT / "README.md"
RELEASE_WORKFLOW = PROJECT_ROOT / ".github" / "workflows" / "release.yml"
RUST_TOOLCHAIN = PROJECT_ROOT / "rust-toolchain.toml"
SHARED_ACTIONS_REVISION = "d400b079fb6a8fa92f7e7b6c57f3d1c92a4b2d54"


def load_toml(path: Path) -> dict[str, object]:
    """Load a TOML file from the repository as a dictionary."""
    return tomllib.loads(path.read_text(encoding="utf-8"))


def test_cargo_config_enables_cranelift_and_mold_linking() -> None:
    """Verify the Cargo profile and Linux linker contract."""
    cargo_config = load_toml(CARGO_CONFIG)
    linux_target = cargo_config["target"]["x86_64-unknown-linux-gnu"]

    assert cargo_config["unstable"]["codegen-backend"] is True
    assert cargo_config["profile"]["dev"]["codegen-backend"] == "cranelift"
    assert linux_target["linker"] == "clang"
    assert linux_target["rustflags"] == ["-C", "link-arg=-fuse-ld=mold"]


def test_toolchain_installs_cranelift_component() -> None:
    """Verify the pinned Rust toolchain includes Cranelift codegen."""
    toolchain = load_toml(RUST_TOOLCHAIN)["toolchain"]

    assert toolchain["channel"].startswith("nightly-")
    assert "rustfmt" in toolchain["components"]
    assert "clippy" in toolchain["components"]
    assert "rustc-codegen-cranelift" in toolchain["components"]


def test_ci_installs_linker_tools_and_uses_coverage_carve_out() -> None:
    """Verify CI installs linker tools and delegates coverage backend handling."""
    workflow = CI_WORKFLOW.read_text(encoding="utf-8")

    assert f"setup-rust@{SHARED_ACTIONS_REVISION}" in workflow
    assert f"generate-coverage@{SHARED_ACTIONS_REVISION}" in workflow
    assert "if: runner.os == 'Linux'" in workflow
    assert "export DEBIAN_FRONTEND=noninteractive" in workflow
    assert (
        "sudo apt-get install --yes --no-install-recommends clang mold"
        in workflow
    )
    assert "CARGO_PROFILE_DEV_CODEGEN_BACKEND" not in workflow


def test_release_workflow_installs_linker_tools() -> None:
    """Verify release builds have the Linux linker prerequisites available."""
    workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")

    assert f"setup-rust@{SHARED_ACTIONS_REVISION}" in workflow
    assert "if: runner.os == 'Linux'" in workflow
    assert "export DEBIAN_FRONTEND=noninteractive" in workflow
    assert (
        "sudo apt-get install --yes --no-install-recommends clang mold"
        in workflow
    )
    assert 'RUSTFLAGS: ""' in workflow


def test_build_configuration_is_developer_documentation() -> None:
    """Verify build-system details live in developer documentation."""
    developer_docs = DEVELOPERS_GUIDE.read_text(encoding="utf-8")
    readme = README.read_text(encoding="utf-8")

    assert "## Build configuration" in developer_docs
    assert "Cranelift code generation backend" in developer_docs
    assert "`clang` with `mold`" in developer_docs
    assert "LLVM backend carve-out" in developer_docs
    assert "nightly-2025-12-10" not in developer_docs
    assert "Toolchain prerequisites" not in readme
    assert "rustc-codegen-cranelift" not in readme
