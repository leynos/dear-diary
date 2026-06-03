"""Shared fixtures for local GitHub Actions workflow validation.

This module defines the `ActRunner` protocol used by workflow tests, the
`SubprocessActRunner` implementation that invokes the external `act` command
with `subprocess.run`, and the `act_runner` pytest fixture that returns an
`ActRunner` with isolated artefact storage.

The fixture assumes `act` is available on `PATH` and that a Docker-compatible
daemon is reachable. Tests call the runner with an event name, job, event
fixture, and optional workflow path, then inspect the returned
`subprocess.CompletedProcess[str]`.

Examples
--------
Use the fixture from a test::

    def test_workflow(act_runner):
        result = act_runner(
            "pull_request",
            job="stage-only",
            event_path=Path("tests/fixtures/selftest.event.json"),
        )
        assert result.returncode == 0
"""

from __future__ import annotations

from collections.abc import Mapping
import os
import subprocess
from dataclasses import dataclass
from pathlib import Path
from types import MappingProxyType

import pytest

from _act_support import PROJECT_ROOT, ActRunner

DEFAULT_RUNNER_IMAGE = "catthehacker/ubuntu:act-latest"


@dataclass(frozen=True)
class SubprocessActRunner:
    """Run `act` with a temporary artefact server directory.

    Parameters supplied to the returned callable select the event, job, and
    workflow. The runner captures `act --json` output for executable jobs so
    tests can assert on structured workflow logs.
    """

    artifact_dir: Path
    runner_label_map: Mapping[str, str]

    def __post_init__(self) -> None:
        """Freeze runner-label mappings against in-place mutation."""
        object.__setattr__(
            self,
            "runner_label_map",
            MappingProxyType(dict(self.runner_label_map)),
        )

    def __call__(
        self,
        event_name: str,
        *,
        job: str,
        event_path: Path,
        workflow: str | Path | None = None,
        list_only: bool = False,
    ) -> subprocess.CompletedProcess[str]:
        """Implement `ActRunner` by invoking `act` as a subprocess.

        Parameters
        ----------
        event_name : str
            GitHub event name passed to `act`, such as `pull_request` or
            `push`.
        job : str
            Workflow job identifier to execute or list.
        event_path : pathlib.Path
            Path to the JSON event payload fixture.
        workflow : str or pathlib.Path, optional
            Workflow file path passed to `act -W`. Defaults to `None`, which
            lets `act` discover matching workflows.
        list_only : bool, optional
            When true, run `act --list` instead of executing the job.

        Returns
        -------
        subprocess.CompletedProcess[str]
            Captured `act` process result including exit code and text output.
        """
        command = [
            "act",
            event_name,
            "-j",
            job,
            "-e",
            str(event_path),
        ]
        for label, image in self.runner_label_map.items():
            command.extend(["-P", f"{label}={image}"])
        if workflow is not None:
            command.extend(["-W", str(workflow)])
        if list_only:
            command.append("--list")
        else:
            self.artifact_dir.mkdir(parents=True, exist_ok=True)
            command.extend(
                [
                    "--artifact-server-path",
                    str(self.artifact_dir),
                    "--json",
                ]
            )
        return subprocess.run(  # noqa: S603 -- hard-coded executable, safe subprocess call with shell=False
            command,
            cwd=PROJECT_ROOT,
            capture_output=True,
            text=True,
            timeout=1200,
            check=False,
        )


@pytest.fixture(scope="module")
def act_runner(tmp_path_factory: pytest.TempPathFactory) -> ActRunner:
    """Return a module-scoped `act` runner with isolated artefact storage.

    The shared artefact directory is safe for modules that generate at most
    one uploaded workflow artefact. Use a narrower fixture scope if a module
    needs to execute multiple artefact-producing jobs.
    """
    runner_image = os.environ.get("ACT_RUNNER_IMAGE", DEFAULT_RUNNER_IMAGE)
    return SubprocessActRunner(
        artifact_dir=tmp_path_factory.mktemp("act-artifacts"),
        runner_label_map={"ubuntu-latest": runner_image},
    )
