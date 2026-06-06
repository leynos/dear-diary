# Debugging Plan: Act Workflow Test Container Startup Failure

**Generated**: 2026-06-06T00:36:19Z **Issue ID**:
act-selftest-container-startup **Severity**: Medium

## Problem Statement

`make test-workflow` is expected to run the local `act` workflow harness and
exercise `.github/workflows/selftest-staging.yml` against a dummy release
binary. It now fails before repository workflow code runs: `act` reaches the
`Set up job` step, asks Podman to create and run the
`catthehacker/ubuntu:act-latest` runner container, then receives a
`conmon failed: exit status 1` container startup error from the daemon. The
failure blocks the full quality gate and prevents validating the new checksum
sidecar assertion added to `tests/test_selftest_stage.py`.

## Context Summary

| Aspect              | Details                                                                                                |
| ------------------- | ------------------------------------------------------------------------------------------------------ |
| First observed      | 2026-06-05 during `make test-workflow` after checksum sidecar assertion was added                      |
| Reproduction rate   | 2/2 attempts in the current shell failed at container startup                                          |
| Affected components | `act` 0.2.88, rootless Podman socket, `catthehacker/ubuntu:act-latest`, `tests/test_selftest_stage.py` |
| Recent changes      | Added checksum sidecar content assertions; no change to the `act` command line or runner image         |

### Error Artefacts

```text
docker create image=catthehacker/ubuntu:act-latest entrypoint=["tail" "-f" "/dev/null"] network="host"
docker run image=catthehacker/ubuntu:act-latest entrypoint=["tail" "-f" "/dev/null"] network="host"
{"level":"error","msg":"failed to start container: Error response from daemon: conmon failed: exit status 1"}
Error: failed to start container: Error response from daemon: conmon failed: exit status 1
```

The failure logs are in:

- `/tmp/test-workflow-dear-diary-chore_plan-shared-actions-binstall-update.out`
- `/tmp/test-workflow-retry-dear-diary-chore_plan-shared-actions-binstall-update.out`

### Information Gaps

- The Podman daemon's detailed event or container logs for the failed `act`
  containers have not yet been collected.
- It is not yet known whether the failure is specific to
  `catthehacker/ubuntu:act-latest`, to `act`'s host-network invocation, or to
  rootless Podman runtime state.
- Other agents have running containers on the same host. They must not be
  stopped or removed during falsification.

______________________________________________________________________

## Hypotheses

### H1: Rootless Podman Runtime State Cannot Start New `act` Containers

**Claim**: The local rootless Podman runtime or `conmon` state is unhealthy for
new containers, independent of the repository workflow.

**Plausibility**: High — `act` fails before checkout or any project command
runs, and the error comes from the container daemon.

**Prediction**: If this hypothesis holds, then a minimal new Podman container
using the same runtime path also fails before executing its command.

#### Falsification Plan

| Step | Action                                                                                                                                                                                         | Expected Negative Result                                                                            |
| ---- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| 1    | Run a non-destructive smoke test such as `podman run --rm --pull=never alpine:latest true` if the image exists locally.                                                                        | The container starts and exits 0, disproving a general Podman start failure.                        |
| 2    | If no local small image exists, run `podman image list --format '{{.Repository}}:{{.Tag}}'` and pick an already-present image for `true` or `bash -lc true`.                                   | Any existing image starts and exits 0, narrowing the issue away from general Podman runtime health. |
| 3    | Inspect `podman events --since 10m --filter event=start --filter event=died` and `podman ps --all --filter ancestor=catthehacker/ubuntu:act-latest --format '{{.ID}} {{.Status}} {{.Names}}'`. | Events show no runtime-wide failures, or failed containers have an image-specific error.            |

**Tooling**: `podman ps`, `podman image list`, `podman run`, `podman events`.

**Confidence on falsification**: High. A successful new container using the
same rootless runtime strongly falsifies a general Podman/conmon outage.

______________________________________________________________________

### H2: The `catthehacker/ubuntu:act-latest` Runner Image Is Broken or Incompatible

**Claim**: The specific runner image used by `ACT_RUNNER_IMAGE` cannot start
under the current Podman/crun setup, even though other images can.

**Plausibility**: High — the failing command starts that exact image and fails
before repository steps run.

**Prediction**: If this hypothesis holds, then directly running the runner
image with the same entrypoint shape fails, while a different existing image
can start.

#### Falsification Plan

| Step | Action                                                                                                                                                                  | Expected Negative Result                                                                                     |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| 1    | Run `podman run --rm --pull=never --entrypoint tail catthehacker/ubuntu:act-latest -f /dev/null` with a short timeout, then stop only that test container if it starts. | The runner image starts successfully, disproving image startup incompatibility.                              |
| 2    | Run `podman inspect catthehacker/ubuntu:act-latest --format '{{.Id}} {{.Architecture}} {{.Os}}'`.                                                                       | The image is present, correctly typed for Linux, and not corrupt.                                            |
| 3    | Try `ACT_RUNNER_IMAGE=<known-good-local-image> make test-workflow` only if step 1 fails and a suitable act-compatible image is already present locally.                 | The alternative image also fails before workflow steps, shifting suspicion back to Podman or act invocation. |

**Tooling**: `podman run`, `podman inspect`, optional `timeout`, existing local
images only unless network pull is explicitly acceptable.

**Confidence on falsification**: Medium-high. Directly starting the image tests
the same image/runtime boundary as `act`, but not all labels, mounts, and
network flags used by `act`.

______________________________________________________________________

### H3: `act`'s Host Network or Artifact-Server Options Trigger the Container Failure

**Claim**: Podman can start the runner image normally, but the exact options
generated by `act` for the selftest job, especially `network="host"` or the
artifact server setup, make container startup fail.

**Plausibility**: Medium — the log shows `network="host"`, and the selftest uses
`--artifact-server-path`; however, this same harness passed earlier.

**Prediction**: If this hypothesis holds, then direct `podman run` without
`--network host` succeeds, while a direct command with host networking fails, or
`act --list` continues to pass while only executable jobs fail.

#### Falsification Plan

| Step | Action                                                                                                                       | Expected Negative Result                                                                   |
| ---- | ---------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| 1    | Run `act pull_request -j stage-only -e tests/fixtures/selftest.event.json -W .github/workflows/selftest-staging.yml --list`. | List mode fails too, disproving that startup-only options are required.                    |
| 2    | Directly start the runner image with and without `--network host`, using the same entrypoint.                                | Both direct starts succeed, weakening the host-network hypothesis.                         |
| 3    | Run a minimal temporary workflow through `act` that has no upload-artifact step and only echoes text.                        | The minimal workflow fails identically, showing artifact-server options are not necessary. |

**Tooling**: `act --list`, `podman run`, a temporary scratch workflow under
`/tmp` if needed.

**Confidence on falsification**: Medium. A minimal workflow isolates `act`
execution from this repository's selftest workflow, but it still depends on
`act`'s runner setup.

______________________________________________________________________

### H4: The Test Code Change Introduced a Workflow-Harness Regression

**Claim**: The checksum sidecar assertion or adjacent test changes introduced a
regression that causes `make test-workflow` to fail.

**Plausibility**: Low — the observed failure occurs before pytest reaches any
checksum assertion and before the workflow checks out source code.

**Prediction**: If this hypothesis holds, then a minimal `act --list` or direct
workflow run that avoids the new assertion would pass, and the failure would
occur after artefacts are extracted.

#### Falsification Plan

| Step | Action                                                                                                                           | Expected Negative Result                                                    |
| ---- | -------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| 1    | Run only `tests/test_release_workflow_dry_run.py` through `make test-workflow`'s uv environment.                                 | The dry-run test passes, showing parser-side harness code is fine.          |
| 2    | Inspect the failing log ordering and confirm no test assertion after `_run_stage_only` executes.                                 | Failure remains at `Set up job`, disproving checksum assertion involvement. |
| 3    | If container startup becomes healthy, rerun `make test-workflow` and observe whether the new checksum assertion passes or fails. | The checksum assertion passes, falsifying this as root cause.               |

**Tooling**:
`uv run --with pytest --with cmd-mox --with pyyaml python -m pytest tests/test_release_workflow_dry_run.py`,
existing failure logs.

**Confidence on falsification**: High. The current failure phase is already
strong evidence against a test-code root cause.

______________________________________________________________________

### H5: Concurrent Containers Exhaust a Rootless Runtime Resource

**Claim**: Other running containers on the shared host exhaust a rootless
Podman resource, causing only new container starts to fail.

**Plausibility**: Medium — multiple unrelated Playwright containers were
running when the failure was observed. The project instructions forbid killing
other agents' processes.

**Prediction**: If this hypothesis holds, then Podman reports high container
count, namespace, storage, lock, or cgroup pressure, and new starts fail until
resources clear.

#### Falsification Plan

| Step | Action                                                                                       | Expected Negative Result                                                    |
| ---- | -------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| 1    | Run `podman ps --format '{{.ID}} {{.Image}} {{.Status}} {{.Names}}'` and `podman system df`. | Resource usage is low and unrelated new containers start successfully.      |
| 2    | Check `df -h . /tmp "$XDG_RUNTIME_DIR"` and `df -i . /tmp "$XDG_RUNTIME_DIR"`.               | Disk space and inodes are healthy, falsifying storage pressure.             |
| 3    | Wait for unrelated containers to exit naturally, then rerun `make test-workflow`.            | Failure persists after resource pressure clears, weakening this hypothesis. |

**Tooling**: `podman ps`, `podman system df`, `df`, passive waiting only.

**Confidence on falsification**: Medium. Resource pressure can be transient, so
a negative result reduces but may not eliminate the hypothesis.

______________________________________________________________________

## Recommended Execution Order

1. **H4** — Cheapest: it uses existing logs and the dry-run pytest target to
   rule out the local checksum assertion as the root cause.
2. **H1** — Most decisive environment check: prove whether any new container
   can start.
3. **H2** — Tests the exact runner image boundary if general Podman health is
   good.
4. **H3** — Isolates `act`-specific options after image/runtime health is
   known.
5. **H5** — Check shared-host resource pressure and rerun after unrelated
   containers clear naturally.

## Termination Criteria

- **Root cause identified**: One hypothesis survives its falsification tests
  while the others are eliminated, and a minimal corrective action makes
  `make test-workflow` pass.
- **Escalation trigger**: All hypotheses are falsified, or Podman continues to
  fail new container starts for reasons that require stopping other agents'
  containers or changing host-level runtime configuration.

## Notes for Executing Agent

- Do not kill or remove containers that were not created for this
  investigation.
- Prefer local images and `--pull=never` for direct Podman smoke tests, so the
  investigation does not introduce network variability.
- Any temporary workflow files should live under `/tmp`, not in the repository.
- Record all commands and outcomes in this document or a short follow-up note
  before changing repository code.

## Falsification Results

### H4: Test Code Change Introduced a Workflow-Harness Regression

**Status**: Falsified.

**Executor**: Bacon, wyvern subagent.

**Evidence**:

- `uv run --with pytest --with cmd-mox --with pyyaml python -m pytest tests/test_release_workflow_dry_run.py`
  passed.
- Both failing workflow logs show failure at `Set up job` with a
  `conmon failed: exit status 1` daemon error.
- The failing traceback is the `assert result.returncode == 0` in
  `_run_stage_only`, before checksum sidecar assertions or post-workflow test
  logic execute.

**Conclusion**: The current evidence points at an environment/runtime container
startup failure, not a repository test-code regression.

### H1: Rootless Podman Runtime State Cannot Start New `act` Containers

**Status**: Falsified.

**Executor**: Lovelace, wyvern subagent.

**Evidence**:

- Local images could start under rootless Podman, including
  `localhost/repovec-integration-tests:latest` and
  `mcr.microsoft.com/playwright:latest`.
- Podman events showed successful container starts after the failing `act`
  run.
- Disk and inode usage were healthy for the repository, `/tmp`, and
  `/run/user/1000`.

**Conclusion**: The runtime itself could start containers. The failure was not
a general Podman or `conmon` outage.

### H2: Runner Image Is Broken or Incompatible

**Status**: Falsified.

**Executor**: Averroes, wyvern subagent.

**Evidence**:

- Direct Podman starts of `catthehacker/ubuntu:act-latest` succeeded.
- The image metadata reported a valid Linux/amd64 image.

**Conclusion**: The runner image can start under the current Podman/crun setup.

### H3: Host Network or Artifact-Server Options Trigger the Failure

**Status**: Mostly falsified; narrowed to `act` backend discovery.

**Executor**: Averroes, wyvern subagent, followed by local validation.

**Evidence**:

- Direct Podman starts with and without host networking succeeded.
- A direct `act` run without `DOCKER_HOST` failed after switching to
  `/var/run/docker.sock`.
- `systemctl --user status podman.socket` showed the rootless Podman Docker
  API socket inactive and `/run/user/1000/podman/podman.sock` absent.
- After `systemctl --user start podman.socket`, direct `act` runs with
  `DOCKER_HOST=unix:///run/user/1000/podman/podman.sock` succeeded.

**Conclusion**: The failing layer was the Docker-compatible Podman socket used
by `act`, not the workflow, runner image, host networking, or artifact server.

### H5: Concurrent Containers Exhaust a Rootless Runtime Resource

**Status**: Weakened.

**Executor**: Lovelace, wyvern subagent.

**Evidence**:

- The host had several running containers, but Podman storage, disk space, and
  inode usage were not under pressure.
- Unrelated direct Podman container starts succeeded.

**Conclusion**: Shared-host load was not the immediate cause. It remains a
general operational risk for local `act` testing, but not the root cause of
this failure.

## Resolution

The issue was resolved locally by starting the user Podman socket:

```bash
systemctl --user start podman.socket
```

The pytest `act` runner now passes an explicit `DOCKER_HOST` when the rootless
Podman socket exists and the caller has not already set a Docker backend. It
also passes `--rm` for executable jobs, so failed `act` runs do not leave stale
workflow containers behind.

Once container startup was healthy, `make test-workflow` reached the checksum
assertion and revealed that the pinned shared action emits the two-space
`sha256sum -c` sidecar format. The selftest assertion and execplan now record
that byte-level contract.
