"""Release artefact packaging helpers.

This module prepares copied release binaries, SHA-256 manifests, and optional
`cargo-binstall` tarballs. It is intentionally independent of Cyclopts and
GitHub Actions so tests can exercise filesystem behaviour directly.

Examples
--------
Prepare one Linux release artefact set::

    from pathlib import Path
    request = ArtifactRequest(
        project_root=Path("."),
        package_name="dear-diary",
        version="0.1.0",
        target="x86_64-unknown-linux-gnu",
        os_name="linux",
        arch="x86_64",
        ext="",
        cargo_binstall_archive=True,
    )
    outputs = prepare_artifacts(request)
"""

from __future__ import annotations

import hashlib
import shutil
import tarfile
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class ArtifactRequest:
    """Configuration required to prepare one release artefact set.

    Parameters
    ----------
    project_root : pathlib.Path
        Repository root containing `target/` and receiving `artifacts/`.
    package_name : str
        Cargo package and binary name, for example `dear-diary`.
    version : str
        Release version without the leading `v`.
    target : str
        Rust target triple used for the release build.
    os_name : str
        Operating system label used in the uploaded binary name.
    arch : str
        Architecture label used in the uploaded binary name.
    ext : str
        Platform executable suffix, such as `.exe` on Windows or an empty
        string on Unix targets.
    cargo_binstall_archive : bool
        Whether to create a `cargo-binstall` tarball alongside the binary.

    Returns
    -------
    ArtifactRequest
        Immutable request object consumed by :func:`prepare_artifacts`.

    Raises
    ------
    None
        Dataclass construction performs no filesystem validation.

    Notes
    -----
    `prepare_artifacts` performs the filesystem side effects described by this
    request.

    Examples
    --------
    >>> from pathlib import Path
    >>> request = ArtifactRequest(
    ...     project_root=Path("."),
    ...     package_name="dear-diary",
    ...     version="0.1.0",
    ...     target="x86_64-unknown-linux-gnu",
    ...     os_name="linux",
    ...     arch="x86_64",
    ...     ext="",
    ...     cargo_binstall_archive=True,
    ... )
    >>> request.package_name
    'dear-diary'
    """

    project_root: Path
    package_name: str
    version: str
    target: str
    os_name: str
    arch: str
    ext: str
    cargo_binstall_archive: bool


def write_sha256_manifest(path: Path) -> Path:
    """Write a SHA-256 manifest containing only the artefact basename.

    Parameters
    ----------
    path : pathlib.Path
        Artefact file to hash.

    Returns
    -------
    pathlib.Path
        Path to the `.sha256` manifest written next to `path`.

    Raises
    ------
    FileNotFoundError
        If `path` does not exist.
    OSError
        If the artefact cannot be read or the manifest cannot be written.

    Notes
    -----
    This function writes a sibling file as a side effect. The manifest uses the
    artefact basename so users can run `sha256sum -c` from the download
    directory.

    Examples
    --------
    >>> from pathlib import Path
    >>> path = Path("artifacts/linux-x86_64/dear-diary-linux-x86_64")
    >>> manifest = write_sha256_manifest(path)
    >>> manifest.name
    'dear-diary-linux-x86_64.sha256'
    """
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    manifest_path = path.with_name(f"{path.name}.sha256")
    manifest_path.write_text(f"{digest}  {path.name}\n", encoding="utf-8")
    return manifest_path


def prepare_artifacts(request: ArtifactRequest) -> list[Path]:
    """Prepare release binaries, checksums, and optional binstall archives.

    Parameters
    ----------
    request : ArtifactRequest
        Release artefact preparation request.

    Returns
    -------
    list[pathlib.Path]
        Paths to every generated artefact and checksum manifest.

    Raises
    ------
    FileNotFoundError
        If the release binary for `request.target` is missing.
    OSError
        If artefact directories, copied binaries, archives, or checksum
        manifests cannot be created.
    tarfile.TarError
        If the `cargo-binstall` archive cannot be written.

    Notes
    -----
    This function creates files under `artifacts/<os>-<arch>` and may
    overwrite existing artefacts with the same names.

    Examples
    --------
    >>> from pathlib import Path
    >>> request = ArtifactRequest(
    ...     project_root=Path("."),
    ...     package_name="dear-diary",
    ...     version="0.1.0",
    ...     target="x86_64-unknown-linux-gnu",
    ...     os_name="linux",
    ...     arch="x86_64",
    ...     ext="",
    ...     cargo_binstall_archive=True,
    ... )
    >>> outputs = prepare_artifacts(request)  # doctest: +SKIP
    >>> len(outputs)  # doctest: +SKIP
    4
    """
    artifact_dir = request.project_root / "artifacts" / (
        f"{request.os_name}-{request.arch}"
    )
    binary_path = (
        request.project_root
        / "target"
        / request.target
        / "release"
        / f"{request.package_name}{request.ext}"
    )
    binary_name = f"{request.package_name}-{request.os_name}-{request.arch}{request.ext}"

    artifact_dir.mkdir(parents=True, exist_ok=True)
    copied_binary = artifact_dir / binary_name
    shutil.copy2(binary_path, copied_binary)

    outputs = [copied_binary, write_sha256_manifest(copied_binary)]
    if request.cargo_binstall_archive:
        archive_path = artifact_dir / (
            f"{request.package_name}-{request.version}-{request.target}.tar.gz"
        )
        with tarfile.open(archive_path, "w:gz") as archive:
            archive.add(binary_path, arcname=f"{request.package_name}{request.ext}")
        outputs.extend([archive_path, write_sha256_manifest(archive_path)])
    return outputs
