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
import yaml

PROJECT_ROOT = Path(__file__).resolve().parents[2]
CARGO_CONFIG = PROJECT_ROOT / ".cargo" / "config.toml"
CI_WORKFLOW = PROJECT_ROOT / ".github" / "workflows" / "ci.yml"
DEVELOPERS_GUIDE = PROJECT_ROOT / "docs" / "developers-guide.md"
README = PROJECT_ROOT / "README.md"
RELEASE_WORKFLOW = PROJECT_ROOT / ".github" / "workflows" / "release.yml"
RUST_TOOLCHAIN = PROJECT_ROOT / "rust-toolchain.toml"
USER_GUIDE = PROJECT_ROOT / "docs" / "users-guide.md"
MAKEFILE = PROJECT_ROOT / "Makefile"
SHARED_ACTIONS_REVISION = "6cec89bac47a21cf756d68d638a9a510998e57f8"
SETUP_RUST_REVISION = "b804b69fa7f978cf9091b9d9bd5481d8ce58c2ea"
THREADS_FLAG = "-Zthreads=8"
MOLD_FLAG = "-Clink-arg=-fuse-ld=mold"


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


SETUP_RUST_PREFIX = "leynos/shared-actions/.github/actions/setup-rust@"
LINKER_INPUTS = ("install-mold", "install-clang-lld")
LINKER_PACKAGES = ("clang", "lld", "mold")


def workflow_steps(workflow: str) -> list[dict[str, object]]:
    """Return every step of every job in a workflow, parsed as YAML.

    Raises
    ------
    AssertionError
        If the workflow has no jobs mapping.
    """
    jobs = yaml.safe_load(workflow).get("jobs")
    assert isinstance(jobs, dict), "workflow must declare jobs"
    return [
        step
        for job in jobs.values()
        for step in job.get("steps", [])
        if isinstance(step, dict)
    ]


def is_hand_installed_linker(script: str) -> bool:
    """Return whether a shell script apt-installs clang, lld or mold by hand."""
    for command in re.sub(r"\\\n\s*", " ", script).splitlines():
        words = re.split(r"[^A-Za-z0-9-]+", command)
        if command.lstrip().startswith("#"):
            continue
        apt = "apt" in words or "apt-get" in words
        if apt and "install" in words and set(words) & set(LINKER_PACKAGES):
            return True
    return False


def assert_linkers_come_from_setup_rust(workflow: str) -> None:
    """Assert every `setup-rust` step installs the linkers, and none by hand.

    Raises
    ------
    AssertionError
        If no step pins `setup-rust`, a pinned step lacks an input or sets it to
        anything but the string `'true'`, an unpinned `setup-rust` reference
        exists, or a step apt-installs a linker.
    """
    steps = workflow_steps(workflow)
    named = [s for s in steps if str(s.get("uses", "")).startswith(SETUP_RUST_PREFIX)]
    pinned = [
        s
        for s in named
        if re.fullmatch(r"[0-9a-f]{40}", str(s["uses"])[len(SETUP_RUST_PREFIX) :])
    ]
    assert pinned, "setup-rust must be pinned to a full commit SHA"
    assert len(named) == len(pinned), "every setup-rust reference must be pinned"
    for step in pinned:
        inputs = step.get("with") or {}
        for name in LINKER_INPUTS:
            assert inputs.get(name) == "true", f"setup-rust must set {name}: 'true'"
    assert not [s for s in steps if is_hand_installed_linker(str(s.get("run", "")))], (
        "no step may apt-install a linker beside setup-rust"
    )


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
        ["make", "-n", "-s", "-B", target, f"BUILD_HOST_OS={host_os}"],
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


def test_debug_build_composes_the_standard_flags() -> None:
    """Verify the debug build rule restates both flags on Linux and not mold elsewhere."""
    linux = make_commands("target/debug/dear-diary", "Linux")
    darwin = make_commands("target/debug/dear-diary", "Darwin")

    assert f"{THREADS_FLAG} {MOLD_FLAG}" in linux
    assert THREADS_FLAG in darwin
    assert MOLD_FLAG not in darwin


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
    assert_linkers_come_from_setup_rust(workflow)
    coverage_step = named_workflow_step(workflow, "Test and Measure Coverage")
    assert "RUSTFLAGS: -D warnings\n" in coverage_step
    assert THREADS_FLAG not in coverage_step
    assert MOLD_FLAG not in coverage_step
    whitaker_step = named_workflow_step(workflow, "Install Whitaker")
    assert "cranelift" not in whitaker_step, "Cranelift is dropped; Whitaker needs no --cranelift"
    assert "CARGO_PROFILE_DEV_CODEGEN_BACKEND" not in workflow


def test_release_workflow_installs_linker_tools_through_setup_rust() -> None:
    """Verify release builds have the Linux linker prerequisites available."""
    workflow = load_text(RELEASE_WORKFLOW)

    assert f"setup-rust@{SETUP_RUST_REVISION}" in workflow
    assert f"stage-release-artefacts@{SHARED_ACTIONS_REVISION}" in workflow
    assert f"cargo_{'bin' 'stall'}_archive" not in workflow
    assert_linkers_come_from_setup_rust(workflow)
    staging_step = named_workflow_step(workflow, "Stage release artefacts")
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
    # The Cranelift exception must name the measurement's channel, which is the
    # pinned one (concordat BD-006), so the guide carries no other nightly.
    pinned = load_toml(RUST_TOOLCHAIN)["toolchain"]["channel"]
    assert set(re.findall(r"nightly-\d{4}-\d{2}-\d{2}", developer_docs)) == {pinned}, (
        "the guide must name only the pinned nightly channel"
    )
    assert "## Core functionality" in readme
    assert "Toolchain prerequisites" not in readme
    assert "rustc-codegen-cranelift" not in readme
    assert "CI and coverage" not in load_text(USER_GUIDE)


SETUP_RUST_USES = (
    "        uses: leynos/shared-actions/.github/actions/setup-rust@"
    "0123456789abcdef0123456789abcdef01234567\n"
)
BOTH_INPUTS = "        with:\n          install-mold: 'true'\n          install-clang-lld: 'true'\n"


def linker_workflow(inputs: str, extra_steps: str = "") -> str:
    """Return a one-job workflow: a pinned setup-rust step, then extra steps."""
    return (
        "jobs:\n  build-test:\n    steps:\n      - name: Setup Rust\n"
        f"{SETUP_RUST_USES}{inputs}{extra_steps}"
    )


def test_linker_helper_accepts_both_inputs() -> None:
    """A pinned step with both inputs and no hand install passes."""
    assert_linkers_come_from_setup_rust(linker_workflow(BOTH_INPUTS))


@pytest.mark.parametrize(
    "inputs",
    [
        pytest.param("", id="no-with-block"),
        pytest.param("        with:\n          install-mold: 'true'\n", id="no-clang-lld"),
        pytest.param(
            "        with:\n          install-clang-lld: 'true'\n", id="no-mold"
        ),
        pytest.param(
            "        with:\n          install-mold: 'false'\n"
            "          install-clang-lld: 'true'\n",
            id="false-value",
        ),
    ],
)
def test_linker_helper_rejects_a_missing_or_false_input(inputs: str) -> None:
    """A missing or false input fails the assertion."""
    with pytest.raises(AssertionError):
        assert_linkers_come_from_setup_rust(linker_workflow(inputs))


@pytest.mark.parametrize(
    "command",
    [
        "sudo apt-get install --yes clang lld",
        "sudo apt install --yes clang lld",
        "sudo apt-get install --yes \\\n            clang mold",
    ],
)
def test_linker_helper_rejects_a_hand_rolled_install(command: str) -> None:
    """An apt step installing a linker fails even beside the inputs."""
    step = f"      - name: Install\n        run: |\n          {command}\n"

    with pytest.raises(AssertionError):
        assert_linkers_come_from_setup_rust(linker_workflow(BOTH_INPUTS, step))


def test_linker_helper_rejects_inputs_under_env() -> None:
    """Inputs outside `with:` are not action inputs."""
    env_block = "        env:\n          install-mold: 'true'\n          install-clang-lld: 'true'\n"

    with pytest.raises(AssertionError):
        assert_linkers_come_from_setup_rust(linker_workflow(env_block))


def test_linker_helper_rejects_an_unpinned_reference_beside_a_pinned_one() -> None:
    """A second, unpinned setup-rust reference cannot hide beside a pinned one."""
    unpinned = (
        "      - name: Other\n        uses: "
        "leynos/shared-actions/.github/actions/setup-rust@main\n"
    )

    with pytest.raises(AssertionError):
        assert_linkers_come_from_setup_rust(linker_workflow(BOTH_INPUTS, unpinned))


def test_linker_helper_rejects_a_folded_scalar_install() -> None:
    """A folded scalar installing a linker is read as one command."""
    folded = (
        "      - name: Install\n        run: >-\n          sudo apt-get install --yes\n"
        "          clang lld\n"
    )

    with pytest.raises(AssertionError):
        assert_linkers_come_from_setup_rust(linker_workflow(BOTH_INPUTS, folded))
