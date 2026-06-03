"""Parse-time smoke tests for the real release workflow.

This module validates `.github/workflows/release.yml` with `act --list`
without running the expensive release builds. The test confirms that the
workflow graph resolves for a tag-push event and that the workflow still
contains the shared `Stage release artefacts` step.

Examples
--------
Run this validation with the project Makefile::

    make test-workflow

Failures indicate a workflow syntax or structure regression that should be
fixed before relying on GitHub-hosted release validation.
"""

from __future__ import annotations

from pathlib import Path

import yaml

from _act_support import PROJECT_ROOT, ActRunner, combined_logs

RELEASE_EVENT = Path("tests/fixtures/release.event.json")
RELEASE_WORKFLOW = Path(".github/workflows/release.yml")


def test_release_workflow_lists_stage_step(
    act_runner: ActRunner,
) -> None:
    """Assert that `act --list` can resolve the release workflow graph."""
    result = act_runner(
        "push",
        job="build",
        event_path=RELEASE_EVENT,
        workflow=RELEASE_WORKFLOW,
        list_only=True,
    )

    logs = combined_logs(result)
    assert result.returncode == 0, logs
    assert any(
        "build" in line and "Release Binary" in line for line in logs.splitlines()
    ), logs
    workflow_text = (PROJECT_ROOT / RELEASE_WORKFLOW).read_text(encoding="utf-8")
    workflow = yaml.safe_load(workflow_text)
    step_names = [
        name
        for step in workflow["jobs"]["build"]["steps"]
        if (name := _step_name(step)) is not None
    ]
    assert "Stage release artefacts" in step_names, (
        f"expected 'Stage release artefacts' in step_names, got: {step_names}"
    )


def _step_name(step: object) -> str | None:
    """Return a workflow step name when the step has a named shape."""
    match step:
        case {"name": str(name)}:
            return name
        case _:
            return None
