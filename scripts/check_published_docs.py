#!/usr/bin/env python3
"""Verify the actual Cargo archive and its examples without inherited local patches."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib


def isolated_environment(workspace: Path) -> dict[str, str]:
    """Preserve network/tool discovery while discarding Cargo/build overrides."""
    environment = {
        key: value for key, value in os.environ.items()
        if not key.startswith("CARGO_") and key not in {
            "RUSTFLAGS", "RUSTDOCFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER",
            "RUSTC", "RUSTDOC", "QUBIT_FS_REGISTRY_DOC_DEPS",
        }
    }
    environment.update({
        "CARGO_HOME": str(workspace / "cargo-home"),
        "CARGO_TARGET_DIR": str(workspace / "target"),
        "RUSTUP_TOOLCHAIN": "1.94.0",
        "QUBIT_FS_REGISTRY_DOC_DEPS": "published",
    })
    return environment


def validate_manifest(manifest: dict) -> None:
    """Reject dependency source overrides while allowing inert documentation metadata."""
    if "patch" in manifest or "replace" in manifest or "workspace" in manifest:
        raise ValueError("published package must not contain patch/replace/workspace overrides")
    scopes = [manifest, *manifest.get("target", {}).values()]
    for scope in scopes:
        for section in ("dependencies", "dev-dependencies", "build-dependencies"):
            for name, dependency in scope.get(section, {}).items():
                if isinstance(dependency, dict) and any(k in dependency for k in ("path", "git", "workspace")):
                    raise ValueError(f"published dependency {name} retains a local/git/workspace source")


def validate_graph(metadata: dict, package: str, root: Path) -> None:
    """Require all dependencies to come from registries, with only the tested archive local."""
    seen: set[str] = set()
    for entry in metadata["packages"]:
        name = entry["name"]
        if entry.get("source") is None:
            if name != package or Path(entry["manifest_path"]).resolve() != root / "Cargo.toml":
                raise ValueError(f"unexpected local dependency: {name}")
        elif not entry["source"].startswith("registry+"):
            raise ValueError(f"non-registry dependency: {name}")
        if name in {"qubit-fs", "qubit-fs-registry", "qubit-fs-local", "qubit-spi"}:
            if name in seen:
                raise ValueError(f"duplicate filesystem package identity: {name}")
            seen.add(name)


def run(cargo: str, arguments: list[str], cwd: Path, environment: dict[str, str], log: Path) -> str:
    """Run one checked operation and retain diagnostics on failure."""
    process = subprocess.run([cargo, *arguments], cwd=cwd, env=environment, text=True, capture_output=True)
    log.write_text(process.stdout + process.stderr)
    if process.returncode:
        # Cargo identifies unavailable package/version requirements in this diagnostic.
        print(process.stderr, file=sys.stderr)
        raise RuntimeError(f"published dependency verification failed ({log.name}); no local fallback was used")
    return process.stdout


def package_source(cargo: str, root: Path, workspace: Path, environment: dict[str, str]) -> None:
    """Package a source manifest without discovering Cargo config above it."""
    command_cwd = workspace / "cargo-command"
    command_cwd.mkdir()
    run(
        cargo,
        [
            "package",
            "--manifest-path",
            str(root / "Cargo.toml"),
            "--allow-dirty",
            "--no-verify",
        ],
        command_cwd,
        environment,
        workspace / "package.log",
    )


def check(root: Path, workspace: Path) -> None:
    """Produce a normalized archive with Cargo, unpack it, and run the shipped examples."""
    manifest = tomllib.loads((root / "Cargo.toml").read_text())
    if "patch" in manifest or "replace" in manifest:
        raise ValueError("source manifest has local overrides; published checks cannot use them")
    package = manifest["package"]["name"]
    version = manifest["package"]["version"]
    environment = isolated_environment(workspace)
    cargo = shutil.which("cargo")
    if cargo is None:
        raise RuntimeError("cargo is required")
    # --no-verify skips only the redundant package build, not dependency resolution.
    package_source(cargo, root, workspace, environment)
    archive = workspace / "target" / "package" / f"{package}-{version}.crate"
    unpacked = workspace / "unpacked"
    unpacked.mkdir()
    with tarfile.open(archive) as source:
        for member in source.getmembers():
            destination = (unpacked / member.name).resolve()
            if not destination.is_relative_to(unpacked) or not (member.isfile() or member.isdir()):
                raise ValueError(f"unexpected archive member: {member.name}")
        source.extractall(unpacked, filter="data")
    extracted = (unpacked / f"{package}-{version}").resolve()
    validate_manifest(tomllib.loads((extracted / "Cargo.toml").read_text()))
    metadata = run(cargo, ["metadata", "--locked", "--format-version", "1"], extracted, environment, workspace / "metadata.log")
    validate_graph(json.loads(metadata), package, extracted)
    run(cargo, ["test", "--locked", "--all-features", "--test", "integration_tests", "test_shipped_markdown_rust_examples_run", "--", "--nocapture"], extracted, environment, workspace / "examples.log")
    print(f"Published archive and examples verified: {package} {version}")


def main() -> int:
    """Retain a uniquely owned diagnostic workspace until a check succeeds."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    args = parser.parse_args()
    workspace = Path(tempfile.mkdtemp(prefix="qubit-registry-published-")).resolve()
    try:
        check(args.root.resolve(), workspace)
    except (RuntimeError, ValueError, OSError, subprocess.SubprocessError, tarfile.TarError) as error:
        print(f"{error}\nDiagnostics preserved at {workspace}", file=sys.stderr)
        return 1
    shutil.rmtree(workspace)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
