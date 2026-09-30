"""Tests for repository build and coverage configuration.

These tests cover infrastructure files that affect Cargo code generation,
linking, and CI coverage behaviour. They keep the build standard (the parallel
frontend and mold, with no codegen backend selected) from drifting silently
because these files are not exercised by Rust unit tests directly.
"""

from __future__ import annotations

import re
import subprocess
import tomllib
from pathlib import Path

import pytest

PROJECT_ROOT = Path(__file__).resolve().parents[2]
CARGO_CONFIG = PROJECT_ROOT / ".cargo" / "config.toml"
CI_WORKFLOW = PROJECT_ROOT / ".github" / "workflows" / "ci.yml"
DEVELOPERS_GUIDE = PROJECT_ROOT / "docs" / "developers-guide.md"
README = PROJECT_ROOT / "README.md"
RELEASE_WORKFLOW = PROJECT_ROOT / ".github" / "workflows" / "release.yml"
RUST_TOOLCHAIN = PROJECT_ROOT / "rust-toolchain.toml"
USER_GUIDE = PROJECT_ROOT / "docs" / "users-guide.md"
MAKEFILE = PROJECT_ROOT / "Makefile"
SHARED_ACTIONS_REVISION = "eff100c965da05e14fd4e07d7ea518408b312cb8"
SETUP_RUST_REVISION = "8a83824b29dfe8f861714b544dc85fb925ed010c"
THREADS_FLAG = "-Zthreads=8"
MOLD_FLAG = "-Clink-arg=-fuse-ld=mold"
HARDENED_CLANG_INSTALL_COMMAND = (
    "apt-get update && sudo apt-get install --yes --no-install-recommends clang"
)
HARDENED_LINKER_INSTALL_COMMAND = (
    "apt-get update && sudo apt-get install --yes --no-install-recommends "
    "clang mold"
)


def named_workflow_step(workflow: str, name: str) -> str:
    """Return one named workflow step block.

    Raises
    ------
    AssertionError
        If `workflow` does not contain exactly one step named `name`.
    """
    pattern = re.compile(
        rf"(?ms)^      - name: {re.escape(name)}\n"
        r".*?(?=^      - (?:name|uses): |^    [A-Za-z_-]+:|\Z)"
    )
    matches = pattern.findall(workflow)
    assert len(matches) == 1
    return matches[0]


def normalise_shell_continuations(script: str) -> str:
    """Collapse shell line continuations so command sequences are assertable.

    Parameters
    ----------
    script
        Shell script text from a workflow step.

    Returns
    -------
    str
        Script text with ``\\ &&`` line continuations collapsed.
    """
    return re.sub(r"\s*\\\n\s*&&\s*", " && ", script)


def load_text(path: Path) -> str:
    """Load a text file from the repository.

    Parameters
    ----------
    path
        Repository file path to read as UTF-8 text.

    Returns
    -------
    str
        The decoded file contents.

    Raises
    ------
    FileNotFoundError
        If `path` does not exist.
    UnicodeDecodeError
        If `path` is not valid UTF-8 text.

    Examples
    --------
    >>> load_text(README).startswith("#")
    True
    """
    return path.read_text(encoding="utf-8")


def load_toml(path: Path) -> dict[str, object]:
    """Load a TOML file from the repository as a dictionary.

    Parameters
    ----------
    path
        Repository TOML file path to read and parse.

    Returns
    -------
    dict[str, object]
        Parsed TOML document contents.

    Raises
    ------
    FileNotFoundError
        If `path` does not exist.
    tomllib.TOMLDecodeError
        If `path` contains invalid TOML.

    Examples
    --------
    >>> load_toml(RUST_TOOLCHAIN)["toolchain"]["components"]
    ['rustfmt', 'clippy']
    """
    return tomllib.loads(load_text(path))


def test_load_text_reports_missing_file(tmp_path: Path) -> None:
    """Expose missing file failures from repository text reads."""
    with pytest.raises(FileNotFoundError):
        load_text(tmp_path / "missing.txt")


def test_load_toml_reports_invalid_toml(tmp_path: Path) -> None:
    """Expose TOML parse failures from repository configuration reads."""
    invalid_toml = tmp_path / "invalid.toml"
    invalid_toml.write_text("not = [valid\n", encoding="utf-8")

    with pytest.raises(tomllib.TOMLDecodeError):
        load_toml(invalid_toml)


def test_cargo_config_carries_the_build_standard() -> None:
    """Verify the Cargo configuration holds the frontend and linker flags."""
    cargo_config = load_toml(CARGO_CONFIG)
    linux_target = cargo_config["target"]["x86_64-unknown-linux-gnu"]

    assert cargo_config["build"]["rustflags"] == [THREADS_FLAG]
    assert linux_target["linker"] == "clang"
    assert linux_target["rustflags"] == [THREADS_FLAG, MOLD_FLAG]


def test_cargo_config_selects_no_codegen_backend() -> None:
    """Refuse a codegen backend: Cranelift cannot link with `-Zthreads=8`."""
    cargo_config = load_toml(CARGO_CONFIG)

    assert "codegen-backend" not in cargo_config.get("unstable", {})
    assert "codegen-backend" not in cargo_config.get("profile", {}).get("dev", {})


def test_toolchain_carries_no_cranelift_component() -> None:
    """Verify the pinned toolchain does not install the Cranelift backend."""
    toolchain = load_toml(RUST_TOOLCHAIN)["toolchain"]

    assert toolchain["channel"].startswith("nightly-")
    assert toolchain["components"] == ["rustfmt", "clippy"]


def make_commands(target: str, host_os: str) -> str:
    """Return the commands `make -n` prints for a target on a given host OS."""
    completed = subprocess.run(  # noqa: S603 - fixed argv; no shell.
        ["make", "-n", "-s", target, f"BUILD_HOST_OS={host_os}"],
        cwd=PROJECT_ROOT,
        capture_output=True,
        check=True,
        text=True,
    )
    return completed.stdout


@pytest.mark.parametrize("target", ["test", "lint"])
def test_development_recipes_compose_the_standard_flags_on_linux(target: str) -> None:
    """Verify Linux recipes assign both flags and keep the caller's RUSTFLAGS."""
    commands = make_commands(target, "Linux")

    assert f'RUSTFLAGS="${{RUSTFLAGS:+$RUSTFLAGS }}' in commands
    assert f"{THREADS_FLAG} {MOLD_FLAG}" in commands
    assert "-D warnings" in commands


@pytest.mark.parametrize("target", ["test", "lint"])
def test_development_recipes_leave_mold_to_linux(target: str) -> None:
    """Verify a non-Linux host keeps its platform linker."""
    commands = make_commands(target, "Darwin")

    assert THREADS_FLAG in commands
    assert MOLD_FLAG not in commands


def test_release_build_assigns_an_empty_rustflags() -> None:
    """Verify a release build takes neither flag, so it uses the stable linker."""
    commands = make_commands("target/release/dear-diary", "Linux")

    assert 'RUSTFLAGS="${RUSTFLAGS-}"' in commands
    assert THREADS_FLAG not in commands
    assert MOLD_FLAG not in commands


def test_ci_installs_mold_through_setup_rust_and_carves_out_coverage() -> None:
    """Verify CI installs mold via setup-rust and keeps coverage off the flags."""
    workflow = load_text(CI_WORKFLOW)

    assert f"setup-rust@{SETUP_RUST_REVISION}" in workflow
    assert f"generate-coverage@{SHARED_ACTIONS_REVISION}" in workflow
    assert "run: make test-scripts" in workflow
    setup_step = named_workflow_step(workflow, "Setup Rust")
    assert "install-mold: 'true'" in setup_step
    linker_step = named_workflow_step(workflow, "Install clang")
    assert "if: runner.os == 'Linux'" in linker_step
    assert "export DEBIAN_FRONTEND=noninteractive" in linker_step
    assert HARDENED_CLANG_INSTALL_COMMAND in normalise_shell_continuations(
        linker_step
    )
    coverage_step = named_workflow_step(workflow, "Test and Measure Coverage")
    assert "RUSTFLAGS: -D warnings\n" in coverage_step
    assert THREADS_FLAG not in coverage_step
    assert MOLD_FLAG not in coverage_step
    assert "whitaker-installer --cranelift" not in workflow
    assert "CARGO_PROFILE_DEV_CODEGEN_BACKEND" not in workflow


def test_release_workflow_installs_linker_tools() -> None:
    """Verify release builds have the Linux linker prerequisites available."""
    workflow = load_text(RELEASE_WORKFLOW)

    assert f"setup-rust@{SHARED_ACTIONS_REVISION}" in workflow
    assert f"stage-release-artefacts@{SHARED_ACTIONS_REVISION}" in workflow
    assert f"cargo_{'bin' 'stall'}_archive" not in workflow
    linker_step = named_workflow_step(workflow, "Install mold linker")
    staging_step = named_workflow_step(workflow, "Stage release artefacts")
    assert "if: runner.os == 'Linux'" in linker_step
    assert "export DEBIAN_FRONTEND=noninteractive" in linker_step
    assert HARDENED_LINKER_INSTALL_COMMAND in normalise_shell_continuations(
        linker_step
    )
    assert "config-file: .github/release-staging.toml" in staging_step
    assert "target: ${{ matrix.key }}" in staging_step
    assert "path: ${{ steps.stage.outputs.artifact-dir }}" in workflow
    assert 'RUSTFLAGS: ""' in workflow


def test_build_configuration_is_developer_documentation() -> None:
    """Verify build-system details live in developer documentation."""
    developer_docs = load_text(DEVELOPERS_GUIDE)
    readme = load_text(README)

    assert "## Build configuration" in developer_docs
    assert "### CI and coverage" in developer_docs
    assert "### Cranelift exception" in developer_docs
    assert "aws_lc_0_45_0_*" in developer_docs
    assert "`clang` with `mold`" in developer_docs
    assert "LLVM instrumentation carve-out" in developer_docs
    assert "shared `generate-coverage` action" in developer_docs
    assert "LLVM coverage" in developer_docs
    assert "instrumentation" in developer_docs
    assert re.search(r"nightly-\d{4}-\d{2}-\d{2}", developer_docs) is None
    assert "## Core functionality" in readme
    assert "Toolchain prerequisites" not in readme
    assert "rustc-codegen-cranelift" not in readme
    assert "CI and coverage" not in load_text(USER_GUIDE)
