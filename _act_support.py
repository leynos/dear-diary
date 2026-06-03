"""Shared support types for local GitHub Actions workflow tests.

The workflow tests and their fixtures need a common project root, a callable
`act` runner protocol, and a helper for combining subprocess logs. Keeping
these symbols outside `conftest.py` lets tests import normal support code
without depending on pytest's fixture-discovery module.
"""

from __future__ import annotations

import subprocess
from pathlib import Path
from typing import Protocol

PROJECT_ROOT = Path(__file__).resolve().parent


def combined_logs(result: subprocess.CompletedProcess[str]) -> str:
    """Return combined standard output and error for assertion messages.

    Parameters
    ----------
    result : subprocess.CompletedProcess[str]
        Completed subprocess result returned by an `ActRunner`.

    Returns
    -------
    str
        Standard output and standard error joined with a newline.
    """
    return f"{result.stdout}\n{result.stderr}"


class ActRunner(Protocol):
    """Callable `act` runner with its artefact directory.

    Attributes
    ----------
    artifact_dir : pathlib.Path
        Host directory where `act` writes uploaded workflow artefacts.
    """

    artifact_dir: Path

    def __call__(
        self,
        event_name: str,
        *,
        job: str,
        event_path: Path,
        workflow: str | Path | None = None,
        list_only: bool = False,
    ) -> subprocess.CompletedProcess[str]:
        """Run `act` for one workflow job.

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
