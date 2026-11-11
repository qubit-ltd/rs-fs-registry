#!/usr/bin/env python3
"""Verify the actual Cargo archive and its examples without inherited local patches."""

from __future__ import annotations

import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import stat
import subprocess
import sys
import tarfile
import tempfile
import time
import tomllib
from types import SimpleNamespace
from ctypes import wintypes

_WIN32_CREATE_SUSPENDED = 0x00000004  # CREATE_SUSPENDED from WinBase.h


_SIBLING_NAMES = {"qubit-fs", "qubit-fs-local", "qubit-local-files"}
_EXACT_VERSION = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")


def load_check_settings(root: Path) -> tuple[str, dict[str, str]]:
    """Load and validate the published check's pinned toolchain and sibling packages."""
    try:
        toolchain = tomllib.loads((root / "rust-toolchain.toml").read_text())
        channel = toolchain["toolchain"]["channel"]
    except (OSError, tomllib.TOMLDecodeError, KeyError, TypeError) as error:
        raise ValueError(f"invalid rust-toolchain.toml [toolchain].channel: {error}") from error
    if not isinstance(channel, str) or not _EXACT_VERSION.fullmatch(channel):
        raise ValueError(f"toolchain channel must be an exact three-part version, found {channel!r}")
    try:
        manifest = tomllib.loads((root / "Cargo.toml").read_text())
        versions = manifest["package"]["metadata"]["documentation"]["published-check"]["sibling-versions"]
    except (OSError, tomllib.TOMLDecodeError, KeyError, TypeError) as error:
        raise ValueError(f"invalid published-check sibling-versions metadata: {error}") from error
    if not isinstance(versions, dict) or set(versions) != _SIBLING_NAMES:
        actual = sorted(versions) if isinstance(versions, dict) else type(versions).__name__
        raise ValueError(f"published-check sibling-versions must contain exactly {sorted(_SIBLING_NAMES)}, found {actual}")
    for name, version in versions.items():
        if not isinstance(version, str) or not _EXACT_VERSION.fullmatch(version):
            raise ValueError(f"published-check sibling version for {name} must be an exact three-part version, found {version!r}")
    return channel, versions


def isolated_environment(workspace: Path, toolchain: str) -> dict[str, str]:
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
        "RUSTUP_TOOLCHAIN": toolchain,
        "QUBIT_FS_REGISTRY_DOC_DEPS": "published",
        "QUBIT_FS_SIBLING_ROOT": str(workspace / "sibling-root"),
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


def validate_graph(metadata: dict, package: str, root: Path,
                   allowed_local_paths: dict[str, Path]) -> None:
    """Require registry dependencies except the archive and exact named sibling overrides."""
    allowed_names = {"qubit-fs", "qubit-fs-local", "qubit-local-files"}
    unexpected_names = set(allowed_local_paths) - allowed_names
    if unexpected_names:
        raise ValueError(f"unrecognized local dependency allowlist entries: {sorted(unexpected_names)}")
    seen: set[str] = set()
    for entry in metadata["packages"]:
        name = entry["name"]
        if entry.get("source") is None:
            manifest_path = Path(entry["manifest_path"]).resolve()
            if name == package and manifest_path == (root / "Cargo.toml").resolve():
                pass
            elif (name in allowed_local_paths
                  and manifest_path == (allowed_local_paths[name] / "Cargo.toml").resolve()):
                sibling_manifest = tomllib.loads((allowed_local_paths[name] / "Cargo.toml").read_text())
                if entry.get("version") != sibling_manifest["package"]["version"]:
                    raise ValueError(f"{name} path override resolved an unexpected version")
            else:
                raise ValueError(f"unexpected local dependency: {name}")
        elif not entry["source"].startswith("registry+"):
            raise ValueError(f"non-registry dependency: {name}")
        if name in {"qubit-fs", "qubit-fs-registry", "qubit-fs-local", "qubit-spi", "qubit-local-files"}:
            if name in seen:
                raise ValueError(f"duplicate filesystem package identity: {name}")
            seen.add(name)


def _windows_job_functions():
    """Create ctypes bindings for the Windows owned-process Job Object APIs."""
    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel32.CreateJobObjectW.argtypes = [ctypes.c_void_p, wintypes.LPCWSTR]
    kernel32.CreateJobObjectW.restype = wintypes.HANDLE
    kernel32.SetInformationJobObject.argtypes = [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD]
    kernel32.SetInformationJobObject.restype = wintypes.BOOL
    kernel32.AssignProcessToJobObject.argtypes = [wintypes.HANDLE, wintypes.HANDLE]
    kernel32.AssignProcessToJobObject.restype = wintypes.BOOL
    kernel32.CreateToolhelp32Snapshot.argtypes = [wintypes.DWORD, wintypes.DWORD]
    kernel32.CreateToolhelp32Snapshot.restype = wintypes.HANDLE
    kernel32.Thread32First.argtypes = [wintypes.HANDLE, ctypes.c_void_p]
    kernel32.Thread32First.restype = wintypes.BOOL
    kernel32.Thread32Next.argtypes = [wintypes.HANDLE, ctypes.c_void_p]
    kernel32.Thread32Next.restype = wintypes.BOOL
    kernel32.OpenThread.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel32.OpenThread.restype = wintypes.HANDLE
    kernel32.ResumeThread.argtypes = [wintypes.HANDLE]
    kernel32.ResumeThread.restype = wintypes.DWORD
    kernel32.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel32.CloseHandle.restype = wintypes.BOOL
    kernel32.TerminateJobObject.argtypes = [wintypes.HANDLE, wintypes.UINT]
    kernel32.TerminateJobObject.restype = wintypes.BOOL
    return kernel32


class WindowsJob:
    """Owns a kill-on-close Job Object and retries failed cleanup safely."""

    def __init__(self, kernel32, handle):
        self._kernel32 = kernel32
        self._handle = handle

    def close(self) -> None:
        if self._handle and not self._kernel32.CloseHandle(self._handle):
            raise ctypes.WinError(ctypes.get_last_error())
        self._handle = None

    def terminate(self) -> None:
        if self._handle and not self._kernel32.TerminateJobObject(self._handle, 1):
            raise ctypes.WinError(ctypes.get_last_error())

    def cleanup(self) -> None:
        try:
            self.close()
            return
        except OSError as first_close_error:
            try:
                self.terminate()
            except OSError as terminate_error:
                try:
                    self.close()
                    return
                except OSError as retry_close_error:
                    raise RuntimeError(
                        f"Job Object CloseHandle failed ({first_close_error}); "
                        f"TerminateJobObject failed ({terminate_error}); "
                        f"CloseHandle retry failed ({retry_close_error}); handle retained for retry"
                    ) from retry_close_error
            try:
                self.close()
            except OSError as retry_close_error:
                raise RuntimeError(
                    f"Job Object CloseHandle failed ({first_close_error}); "
                    f"TerminateJobObject terminated the process tree, but CloseHandle retry failed "
                    f"({retry_close_error}); handle retained for retry"
                ) from retry_close_error

    def __del__(self):
        # The explicit cleanup path reports failures; destruction makes one
        # final best-effort attempt while this object still owns the handle.
        try:
            self.cleanup()
        except Exception:
            pass


def _close_windows_handle(kernel32, handle, label: str) -> None:
    if not kernel32.CloseHandle(handle):
        error = ctypes.WinError(ctypes.get_last_error())
        raise OSError(f"failed to close Windows {label}: {error}") from error


def _kill_and_reap_process(process: subprocess.Popen, *, wait_seconds: float = 5,
                           deadline: float | None = None, final_deadline: float | None = None) -> str:
    """Best-effort terminate/reap with a bounded wait and diagnostic result."""
    if deadline is None:
        deadline = time.monotonic() + wait_seconds
    if final_deadline is None:
        final_deadline = deadline + 1
    errors = []
    try:
        if process.poll() is None:
            process.kill()
    except Exception as error:
        errors.append(f"kill failed: {error}")
    try:
        process.wait(timeout=max(0, deadline - time.monotonic()))
    except subprocess.TimeoutExpired:
        errors.append("wait exceeded cleanup deadline")
        try:
            process.kill()
        except Exception as error:
            errors.append(f"retry kill failed: {error}")
        try:
            process.wait(timeout=min(1, max(0, final_deadline - time.monotonic())))
        except Exception as error:
            errors.append(f"final bounded wait failed: {error}")
    except Exception as error:
        errors.append(f"wait failed: {error}")
    return "; ".join(errors) if errors else "process reaped"


def _configure_windows_job(kernel32, process: subprocess.Popen):
    """Assign a suspended process to a kill-on-close job, then resume it."""
    class BasicLimitInformation(ctypes.Structure):
        _fields_ = [("PerProcessUserTimeLimit", ctypes.c_longlong),
                    ("PerJobUserTimeLimit", ctypes.c_longlong), ("LimitFlags", wintypes.DWORD),
                    ("MinimumWorkingSetSize", ctypes.c_size_t), ("MaximumWorkingSetSize", ctypes.c_size_t),
                    ("ActiveProcessLimit", wintypes.DWORD), ("Affinity", ctypes.c_size_t),
                    ("PriorityClass", wintypes.DWORD), ("SchedulingClass", wintypes.DWORD)]

    class IoCounters(ctypes.Structure):
        _fields_ = [(name, ctypes.c_ulonglong) for name in
                    ("ReadOperationCount", "WriteOperationCount", "OtherOperationCount",
                     "ReadTransferCount", "WriteTransferCount", "OtherTransferCount")]

    class ExtendedLimitInformation(ctypes.Structure):
        _fields_ = [("BasicLimitInformation", BasicLimitInformation), ("IoInfo", IoCounters),
                    ("ProcessMemoryLimit", ctypes.c_size_t), ("JobMemoryLimit", ctypes.c_size_t),
                    ("PeakProcessMemoryUsed", ctypes.c_size_t), ("PeakJobMemoryUsed", ctypes.c_size_t)]

    job = kernel32.CreateJobObjectW(None, None)
    if not job:
        raise ctypes.WinError(ctypes.get_last_error())
    owned_job = WindowsJob(kernel32, job)
    try:
        limits = ExtendedLimitInformation()
        limits.BasicLimitInformation.LimitFlags = 0x2000  # JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        if not kernel32.SetInformationJobObject(job, 9, ctypes.byref(limits), ctypes.sizeof(limits)):
            raise ctypes.WinError(ctypes.get_last_error())
        if not kernel32.AssignProcessToJobObject(job, wintypes.HANDLE(process._handle)):
            raise ctypes.WinError(ctypes.get_last_error())
        class ThreadEntry(ctypes.Structure):
            _fields_ = [("dwSize", wintypes.DWORD), ("cntUsage", wintypes.DWORD),
                        ("th32ThreadID", wintypes.DWORD), ("th32OwnerProcessID", wintypes.DWORD),
                        ("tpBasePri", wintypes.LONG), ("tpDeltaPri", wintypes.LONG),
                        ("dwFlags", wintypes.DWORD)]

        snapshot = kernel32.CreateToolhelp32Snapshot(0x00000004, 0)  # TH32CS_SNAPTHREAD
        invalid_handle = ctypes.c_void_p(-1).value
        if not snapshot or ctypes.cast(snapshot, ctypes.c_void_p).value == invalid_handle:
            raise ctypes.WinError(ctypes.get_last_error())
        try:
            entry = ThreadEntry()
            entry.dwSize = ctypes.sizeof(entry)
            found = kernel32.Thread32First(snapshot, ctypes.byref(entry))
            thread = None
            while found:
                if entry.th32OwnerProcessID == process.pid:
                    thread = kernel32.OpenThread(0x0002, False, entry.th32ThreadID)  # THREAD_SUSPEND_RESUME
                    if thread:
                        break
                found = kernel32.Thread32Next(snapshot, ctypes.byref(entry))
            if not thread:
                raise ctypes.WinError(ctypes.get_last_error())
            try:
                if kernel32.ResumeThread(thread) == 0xFFFFFFFF:
                    raise ctypes.WinError(ctypes.get_last_error())
            finally:
                _close_windows_handle(kernel32, thread, "primary thread")
        finally:
            _close_windows_handle(kernel32, snapshot, "thread snapshot")
        return owned_job
    except BaseException as setup_error:
        # The process has not resumed. Closing an assigned job terminates it;
        # before assignment, terminate the still-suspended direct child.
        cleanup_error = None
        try:
            owned_job.cleanup()
        except Exception as error:
            cleanup_error = error
        rollback = _kill_and_reap_process(process)
        if cleanup_error is not None or rollback != "process reaped":
            raise RuntimeError(
                f"Windows Job Object setup failed ({setup_error}); rollback cleanup: "
                f"{cleanup_error or 'ok'}; suspended child rollback: {rollback}"
            ) from setup_error
        raise


def close_windows_job(job_handle) -> None:
    if job_handle:
        job_handle.cleanup()


def terminate_process_tree(process: subprocess.Popen, job_handle=None, *,
                           deadline: float | None = None, final_deadline: float | None = None) -> str:
    """Terminate the owned process tree and reap its direct child."""
    if deadline is None:
        deadline = time.monotonic() + 5
    if final_deadline is None:
        final_deadline = deadline + 1
    if os.name == "nt":
        close_windows_job(job_handle)
        reap = _kill_and_reap_process(process, deadline=deadline, final_deadline=final_deadline)
        if reap != "process reaped":
            raise RuntimeError(f"owned Job Object closed; direct child reap failed: {reap}")
        return "owned Job Object closed; process tree terminated and direct child reaped"

    group = process.pid
    errors = []
    try:
        os.killpg(group, signal.SIGTERM)
    except ProcessLookupError:
        pass
    except OSError as error:
        errors.append(f"SIGTERM failed: {error}")
    # Keep the existing graceful window inside the first five-second budget.
    time.sleep(min(0.1, max(0, deadline - time.monotonic())))
    try:
        os.killpg(group, signal.SIGKILL)
    except ProcessLookupError:
        pass
    except OSError as error:
        errors.append(f"SIGKILL failed: {error}")
    reap = _kill_and_reap_process(process, deadline=deadline, final_deadline=final_deadline)
    if reap != "process reaped":
        errors.append(f"direct child reap failed: {reap}")
    if errors:
        raise RuntimeError("; ".join(errors))
    return "process group terminated and direct child reaped"


def run(cargo: str, arguments: list[str], cwd: Path, environment: dict[str, str], log: Path,
        *, timeout_seconds: float, context: str = "published documentation check") -> str:
    """Run one checked operation and retain diagnostics on failure."""
    command = [cargo, *arguments]
    with tempfile.TemporaryFile(mode="w+t", encoding="utf-8") as stdout, \
            tempfile.TemporaryFile(mode="w+t", encoding="utf-8") as stderr:
        try:
            if os.name == "nt":
                process = subprocess.Popen(command, cwd=cwd, env=environment, text=True,
                                           stdout=stdout, stderr=stderr,
                                           creationflags=_WIN32_CREATE_SUSPENDED)
                tree_job = _configure_windows_job(_windows_job_functions(), process)
            else:
                process = subprocess.Popen(command, cwd=cwd, env=environment, text=True,
                                           stdout=stdout, stderr=stderr, start_new_session=True)
                tree_job = None
        except Exception as error:
            rollback = "not needed"
            if "process" in locals() and os.name == "nt":
                rollback = _kill_and_reap_process(process)
            log.write_text("")
            raise RuntimeError(
                f"{context} could not start {command!r}; log: {log}: {error}; rollback: {rollback}"
            ) from error
        deadline = time.monotonic() + timeout_seconds
        timed_out = False
        while process.poll() is None:
            if time.monotonic() >= deadline:
                timed_out = True
                break
            time.sleep(0.05)
        cleanup = "not required"
        cleanup_error = None
        if timed_out:
            cleanup_deadline = time.monotonic() + 5
            final_deadline = cleanup_deadline + 1
            try:
                cleanup = terminate_process_tree(
                    process, tree_job, deadline=cleanup_deadline, final_deadline=final_deadline,
                )
                tree_job = None
            except Exception as error:
                cleanup_error = str(error)
                fallback = _kill_and_reap_process(
                    process, deadline=cleanup_deadline, final_deadline=final_deadline,
                )
                if fallback != "process reaped":
                    cleanup_error += f"; direct-child fallback: {fallback}"
                if os.name == "nt" and tree_job:
                    try:
                        close_windows_job(tree_job)
                        tree_job = None
                    except Exception as retry_error:
                        cleanup_error += f"; Job Object close retry failed: {retry_error}"
                cleanup = f"failed: {cleanup_error}"
        elif os.name != "nt":
            # Cargo normally leaves no descendants, but close any process that
            # inherited this dedicated session before reporting completion.
            try:
                os.killpg(process.pid, signal.SIGTERM)
                time.sleep(0.1)
                os.killpg(process.pid, signal.SIGKILL)
                cleanup = "residual process group terminated"
            except ProcessLookupError:
                pass
            except OSError as error:
                cleanup_error = str(error)
                cleanup = f"failed: {cleanup_error}"
        else:
            try:
                close_windows_job(tree_job)
                tree_job = None
                cleanup = "owned Job Object closed; residual descendants terminated"
            except Exception as error:
                cleanup_error = str(error)
                if tree_job:
                    try:
                        close_windows_job(tree_job)
                        tree_job = None
                    except Exception as retry_error:
                        cleanup_error += f"; Job Object close retry failed: {retry_error}"
                cleanup = f"failed: {cleanup_error}"
        stdout.seek(0)
        stderr.seek(0)
        out = stdout.read()
        err = stderr.read()
        log.write_text(out + err)
    if timed_out:
        raise RuntimeError(
            f"{context} timed out after {timeout_seconds}s: {command!r}; log: {log}; cleanup: {cleanup}"
        )
    if cleanup_error:
        raise RuntimeError(f"{context} cleanup failed: {cleanup_error}; log: {log}")
    if process.returncode:
        print(err, file=sys.stderr)
        raise RuntimeError(
            f"{context} failed with exit code {process.returncode} ({log}); cleanup: {cleanup}; "
            "no local fallback was used"
        )
    return out


def configure_qubit_fs_override(environment: dict[str, str], fs_path: Path) -> None:
    """Write the sole Cargo config override into this check's isolated Cargo home."""
    cargo_home = Path(environment["CARGO_HOME"])
    cargo_home.mkdir(parents=True, exist_ok=True)
    (cargo_home / "config.toml").write_text(
        "[patch.crates-io]\nqubit-fs = { path = " + json.dumps(str(fs_path)) + " }\n"
    )


def sibling_fs_path(root: Path) -> Path:
    """Find the qubit-fs sibling checkout from either the main checkout or a worktree."""
    for parent in root.parents:
        candidate = (parent / "rs-fs").resolve()
        if (candidate / "Cargo.toml").is_file():
            return candidate
    raise ValueError(f"required sibling qubit-fs path override is missing above {root}")


def _collect_git_inputs(root: Path) -> dict[str, tuple[bytes, bool]]:
    """Read Git-visible regular inputs and their build-relevant executable bits."""
    root = root.resolve()
    result = subprocess.run(
        ["git", "-C", str(root), "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    )
    paths = set(filter(None, result.stdout.decode("utf-8", errors="surrogateescape").split("\0")))
    staged = subprocess.run(
        ["git", "-C", str(root), "ls-files", "--stage", "-z"],
        check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    )
    indexed_executable = {}
    for entry in filter(None, staged.stdout.decode("utf-8", errors="surrogateescape").split("\0")):
        metadata, indexed_name = entry.split("\t", 1)
        indexed_executable[indexed_name] = metadata.startswith("100755 ")
    paths.add("Cargo.lock")
    inputs = {}
    for name in sorted(paths):
        relative = Path(name)
        if relative.is_absolute() or any(part in {"", ".", ".."} for part in relative.parts):
            raise ValueError(f"invalid Git input path: {name}")
        if any(part in {".git", "target", ".cargo"} for part in relative.parts):
            continue
        source = root / relative
        cursor = root
        for part in relative.parts:
            cursor /= part
            if cursor.is_symlink():
                raise ValueError(f"symbolic link in package input: {relative}")
        if not source.exists():  # A tracked file may have been deleted in the working tree.
            continue
        if not source.is_file():
            continue
        mode = source.stat().st_mode
        if not stat.S_ISREG(mode):
            raise ValueError(f"non-regular package input: {relative}")
        inputs[relative.as_posix()] = (
            source.read_bytes(),
            bool(mode & (stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH))
            or indexed_executable.get(relative.as_posix(), False),
        )
    return inputs


def _input_digest(inputs: dict[str, tuple[bytes, bool]]) -> str:
    """Hash sorted paths, lengths, content, and executable state without ambiguity."""
    digest = hashlib.sha256()
    digest.update(len(inputs).to_bytes(8, "big"))
    for name, (content, executable) in sorted(inputs.items()):
        path = os.fsencode(name)
        digest.update(len(path).to_bytes(8, "big"))
        digest.update(path)
        digest.update(len(content).to_bytes(8, "big"))
        digest.update(content)
        digest.update(b"\x01" if executable else b"\x00")
    return digest.hexdigest()


def _copy_git_inputs(root: Path, destination: Path, expected_digest: str) -> None:
    """Copy inputs only if their content is stable and matches the prepared evidence."""
    root = root.resolve()
    before = _collect_git_inputs(root)
    before_digest = _input_digest(before)
    if before_digest != expected_digest:
        raise ValueError(f"source inputs changed before snapshot copy: {root}")
    destination.mkdir(parents=True, exist_ok=True)
    for name, (content, executable) in sorted(before.items()):
        source = root / name
        target = destination / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
        if executable:
            target.chmod(target.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
        else:
            target.chmod(target.stat().st_mode & ~(stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH))
    after_digest = _input_digest(_collect_git_inputs(root))
    if after_digest != before_digest:
        raise ValueError(f"source inputs changed while snapshotting: {root}")
    snapshot_inputs = {}
    for name in before:
        target = destination / name
        if target.is_symlink() or not target.is_file():
            raise ValueError(f"snapshot input missing or non-regular: {target}")
        mode = target.stat().st_mode
        snapshot_inputs[name] = (
            target.read_bytes(),
            before[name][1] if os.name == "nt" else
            bool(mode & (stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)),
        )
    snapshot_files = {
        path.relative_to(destination).as_posix()
        for path in destination.rglob("*")
        if path.is_file() and not path.is_symlink()
    }
    if snapshot_files != set(before):
        raise ValueError(f"snapshot input set does not match source inputs: {root}")
    if _input_digest(snapshot_inputs) != before_digest:
        raise ValueError(f"snapshot content does not match source inputs: {root}")


def _source_record(name: str, source: Path, snapshot: Path,
                   inputs: dict[str, tuple[bytes, bool]]) -> dict:
    """Build safe provenance evidence for the input manifest."""
    try:
        head = subprocess.run(
            ["git", "-C", str(source), "rev-parse", "HEAD"], check=True,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        ).stdout.strip()
        dirty = bool(subprocess.run(
            ["git", "-C", str(source), "status", "--porcelain", "--untracked-files=all"],
            check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        ).stdout)
    except subprocess.CalledProcessError as error:
        raise ValueError(f"cannot read Git provenance for {source}: {error}") from error
    manifest_path = source / "Cargo.toml"
    try:
        package = tomllib.loads(manifest_path.read_text())["package"]
        package_name, version = package["name"], package["version"]
    except (OSError, tomllib.TOMLDecodeError, KeyError, TypeError) as error:
        raise ValueError(f"invalid package manifest for {name}: {error}") from error
    return {
        "name": package_name,
        "version": version,
        "source_path": str(source.resolve()),
        "snapshot_path": str(snapshot.resolve()),
        "git_head": head,
        "dirty": dirty,
        "sha256": _input_digest(inputs),
    }


def _validate_snapshot_path_dependencies(source: Path, snapshot: Path, fs_source: Path,
                                         fs_snapshot: Path) -> None:
    """Require every source path dependency to map to the prepared sibling layout."""
    manifest = tomllib.loads((source / "Cargo.toml").read_text())
    expected = (snapshot.parent / "rs-fs").resolve()
    scopes = [manifest, *manifest.get("target", {}).values()]
    if isinstance(manifest.get("workspace"), dict):
        scopes.append({"dependencies": manifest["workspace"].get("dependencies", {})})
    for scope in scopes:
        for section in ("dependencies", "dev-dependencies", "build-dependencies"):
            for name, dependency in scope.get(section, {}).items():
                if not isinstance(dependency, dict) or "path" not in dependency:
                    continue
                path = (source / dependency["path"]).resolve()
                if name != "qubit-fs" or path != fs_source.resolve() or expected != fs_snapshot.resolve():
                    raise ValueError(f"unsupported path dependency {name}: {dependency['path']}")


def prepare_package_source(root: Path, workspace: Path, fs_path: Path,
                           fs_local_path: Path, local_files_path: Path) -> SimpleNamespace:
    """Prepare one stable four-project source snapshot for all published checks."""
    package_root = workspace / "package-input"
    paths = {
        "registry": (root, package_root / "rs-fs-registry"),
        "fs": (fs_path, package_root / "rs-fs"),
        "fs_local": (fs_local_path, package_root / "rs-fs-local"),
        "local_files": (local_files_path, package_root / "rs-local-files"),
    }
    collected = {key: _collect_git_inputs(source) for key, (source, _) in paths.items()}
    records = tuple(
        _source_record(key, source, snapshot, collected[key])
        for key, (source, snapshot) in paths.items()
    )
    (workspace / "input-manifest.json").write_text(
        json.dumps({"inputs": records}, indent=2, sort_keys=True) + "\n"
    )
    record_digests = {record["name"]: record["sha256"] for record in records}
    for key, (source, snapshot) in paths.items():
        package_name = tomllib.loads((source / "Cargo.toml").read_text())["package"]["name"]
        _copy_git_inputs(source, snapshot, record_digests[package_name])
    _validate_snapshot_path_dependencies(root, paths["registry"][1], fs_path, paths["fs"][1])
    _validate_snapshot_path_dependencies(fs_path, paths["fs"][1], fs_path, paths["fs"][1])
    return SimpleNamespace(
        registry=paths["registry"][1], fs=paths["fs"][1],
        fs_local=paths["fs_local"][1], local_files=paths["local_files"][1], records=records,
    )


def package_source(cargo: str, root: Path, workspace: Path, environment: dict[str, str],
                   fs_path: Path) -> None:
    """Package an already prepared source snapshot without changing its inputs."""
    command_cwd = workspace / "cargo-command"
    command_cwd.mkdir()
    configure_qubit_fs_override(environment, fs_path)
    run(
        cargo,
        ["package", "--manifest-path", str(root / "Cargo.toml"), "--allow-dirty", "--no-verify"],
        command_cwd,
        environment,
        workspace / "package.log",
        timeout_seconds=900,
        context="Cargo package",
    )


def check(root: Path, workspace: Path) -> None:
    """Produce a normalized archive with Cargo, unpack it, and run the shipped examples."""
    fs_path = sibling_fs_path(root)
    fs_local_path = (fs_path.parent / "rs-fs-local").resolve()
    local_files_path = (fs_path.parent / "rs-local-files").resolve()
    for label, path in (("qubit-fs-local", fs_local_path), ("qubit-local-files", local_files_path)):
        if not (path / "Cargo.toml").is_file():
            raise ValueError(f"required sibling {label} path source is missing: {path}")
    prepared = prepare_package_source(root, workspace, fs_path, fs_local_path, local_files_path)
    toolchain, expected_versions = load_check_settings(prepared.registry)
    manifest = tomllib.loads((prepared.registry / "Cargo.toml").read_text())
    if "patch" in manifest or "replace" in manifest:
        raise ValueError("source manifest has local overrides; published checks cannot use them")
    package = manifest["package"]["name"]
    version = manifest["package"]["version"]
    fs_version = tomllib.loads((prepared.fs / "Cargo.toml").read_text())["package"]["version"]
    if fs_version != expected_versions["qubit-fs"]:
        raise ValueError(f"qubit-fs expected {expected_versions['qubit-fs']}, actual {fs_version}")
    fs_local_version = tomllib.loads((prepared.fs_local / "Cargo.toml").read_text())["package"]["version"]
    if fs_local_version != expected_versions["qubit-fs-local"]:
        raise ValueError(f"qubit-fs-local expected {expected_versions['qubit-fs-local']}, actual {fs_local_version}")
    local_files_version = tomllib.loads((prepared.local_files / "Cargo.toml").read_text())["package"]["version"]
    if local_files_version != expected_versions["qubit-local-files"]:
        raise ValueError(f"qubit-local-files expected {expected_versions['qubit-local-files']}, actual {local_files_version}")
    environment = isolated_environment(workspace, toolchain)
    environment["QUBIT_FS_SIBLING_ROOT"] = str(prepared.registry.parent)
    configure_qubit_fs_override(environment, prepared.fs)
    cargo = shutil.which("cargo")
    if cargo is None:
        raise RuntimeError("cargo is required")
    # --no-verify skips only the redundant package build, not dependency resolution.
    package_source(cargo, prepared.registry, workspace, environment, prepared.fs)
    archive = workspace / "target" / "package" / f"{package}-{version}.crate"
    unpacked = workspace / "unpacked"
    unpacked.mkdir()
    unpacked_root = unpacked.resolve()
    with tarfile.open(archive) as source:
        for member in source.getmembers():
            destination = (unpacked_root / member.name).resolve()
            if not destination.is_relative_to(unpacked_root) or not (member.isfile() or member.isdir()):
                raise ValueError(f"unexpected archive member: {member.name}")
        source.extractall(unpacked_root, filter="data")
    extracted = (unpacked / f"{package}-{version}").resolve()
    validate_manifest(tomllib.loads((extracted / "Cargo.toml").read_text()))
    metadata = run(cargo, ["metadata", "--locked", "--format-version", "1"], extracted, environment,
                   workspace / "metadata.log", timeout_seconds=180, context="Cargo metadata")
    validate_graph(json.loads(metadata), package, extracted, {
        "qubit-fs": prepared.fs,
        "qubit-fs-local": prepared.fs_local,
        "qubit-local-files": prepared.local_files,
    })
    run(cargo, ["test", "--locked", "--doc"], extracted, environment,
        workspace / "doctests-default.log", timeout_seconds=900,
        context="Cargo published default doctests")
    run(cargo, ["test", "--locked", "--doc", "--all-features"], extracted, environment,
        workspace / "doctests-all-features.log", timeout_seconds=900,
        context="Cargo published all-feature doctests")
    run(cargo, ["test", "--locked", "--all-features", "--test", "integration_tests", "test_shipped_markdown_rust_examples_run", "--", "--nocapture"], extracted, environment,
        workspace / "examples.log", timeout_seconds=3600, context="Cargo published examples")
    provenance = "; ".join(
        f"{record['name']}={record['version']}@{record['git_head']} "
        f"dirty={str(record['dirty']).lower()} sha256={record['sha256']}"
        for record in prepared.records
    )
    print(f"Published archive and examples verified: {package} {version} "
          f"(fixed sibling snapshots: qubit-fs {fs_version}, qubit-fs-local {fs_local_version}, "
          f"qubit-local-files {local_files_version}; snapshot paths: "
          f"{', '.join(record['name'] + '=' + record['snapshot_path'] for record in prepared.records)}; "
          f"all checks use the prepared snapshots; inputs: {provenance})")


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
