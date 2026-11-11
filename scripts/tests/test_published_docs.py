"""Regression tests for the isolation gate, without network or filesystem mutations."""
import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import tarfile
import time
import unittest
from contextlib import redirect_stderr
from contextlib import redirect_stdout
from io import StringIO
from types import SimpleNamespace
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("published_docs", Path(__file__).parents[1] / "check_published_docs.py")
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


def make_sibling_sources(root):
    root = Path(root)
    packages = {
        "rs-fs": ("qubit-fs", "0.3.0"),
        "rs-fs-local": ("qubit-fs-local", "0.11.0"),
        "rs-local-files": ("qubit-local-files", "0.5.0"),
    }
    paths = {}
    for directory, (name, version) in packages.items():
        package_path = root / directory
        package_path.mkdir(parents=True)
        initialize_git(package_path)
        (package_path / "Cargo.toml").write_text(
            f"[package]\nname = '{name}'\nversion = '{version}'\n"
        )
        subprocess.run(["git", "-C", str(package_path), "add", "Cargo.toml"], check=True)
        subprocess.run(["git", "-C", str(package_path), "commit", "-qm", "inputs"], check=True)
        paths[name] = package_path
    return paths


def initialize_git(root):
    subprocess.run(["git", "init", "-q", str(root)], check=True)
    subprocess.run(["git", "-C", str(root), "config", "user.email", "test@example.invalid"], check=True)
    subprocess.run(["git", "-C", str(root), "config", "user.name", "Test"], check=True)
    (root / ".gitignore").write_text("ignored.txt\ntarget/\n.cargo/\n")
    subprocess.run(["git", "-C", str(root), "add", ".gitignore"], check=True)
    subprocess.run(["git", "-C", str(root), "commit", "-qm", "fixture"], check=True)


class PublishedDocsTests(unittest.TestCase):
    def test_load_check_settings_reads_exact_toolchain_and_sibling_versions(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "1.95.2"\n')
            manifest = root / "Cargo.toml"
            manifest.write_text("""[package.metadata.documentation.published-check.sibling-versions]
qubit-fs = "0.4.2"
qubit-fs-local = "0.12.3"
qubit-local-files = "0.6.4"
""")
            self.assertEqual(checker.load_check_settings(root), ("1.95.2", {
                "qubit-fs": "0.4.2", "qubit-fs-local": "0.12.3", "qubit-local-files": "0.6.4",
            }))

    def test_load_check_settings_rejects_invalid_configuration(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "1.94.0"\n')
            valid = {"qubit-fs": "0.3.0", "qubit-fs-local": "0.11.0", "qubit-local-files": "0.5.0"}
            for invalid in [
                {}, {**valid, "other": "1.0.0"}, {**valid, "qubit-fs": ""},
                {**valid, "qubit-fs": 3}, {**valid, "qubit-fs": "0.3"},
            ]:
                (root / "Cargo.toml").write_text("[package.metadata.documentation.published-check.sibling-versions]\n" +
                    "\n".join(f'{key} = {__import__("json").dumps(value)}' for key, value in invalid.items()))
                with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                    checker.load_check_settings(root)
            (root / "Cargo.toml").write_text("[package.metadata.documentation.published-check.sibling-versions]\n" +
                "\n".join(f'{key} = {__import__("json").dumps(value)}' for key, value in valid.items()))
            for channel in ["", 3, "stable", "1.94"]:
                (root / "rust-toolchain.toml").write_text(
                    "[toolchain]\nchannel = " + __import__("json").dumps(channel))
                with self.subTest(channel=channel), self.assertRaises(ValueError):
                    checker.load_check_settings(root)

    def test_check_reports_configured_expected_and_actual_sibling_versions(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "registry"
            root.mkdir()
            (root / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "1.95.2"\n')
            (root / "Cargo.toml").write_text("""[package]
name = "qubit-fs-registry"
version = "0.8.0"
[package.metadata.documentation.published-check.sibling-versions]
qubit-fs = "0.4.2"
qubit-fs-local = "0.11.0"
qubit-local-files = "0.5.0"
""")
            siblings = make_sibling_sources(Path(directory) / "siblings")
            workspace = Path(directory) / "workspace"
            workspace.mkdir()
            with patch.object(checker, "sibling_fs_path", return_value=siblings["qubit-fs"]):
                with self.assertRaisesRegex(ValueError, r"qubit-fs expected 0\.4\.2, actual 0\.3\.0"):
                    checker.check(root, workspace)

    def test_windows_run_assigns_suspended_process_to_job_before_polling(self):
        events = []

        class FakeProcess:
            returncode = 0

            def __init__(self, *args, creationflags, **kwargs):
                events.append(("spawn", creationflags))
                self.pid = 123
                self._handle = 456

            def poll(self):
                events.append(("poll", None))
                return self.returncode

        fake_subprocess = SimpleNamespace(Popen=FakeProcess,
                                          TimeoutExpired=checker.subprocess.TimeoutExpired)
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(checker, "os", SimpleNamespace(name="nt")), \
                    patch.object(checker, "subprocess", fake_subprocess), \
                    patch.object(checker, "_windows_job_functions", return_value=object()), \
                    patch.object(checker, "_configure_windows_job",
                                 side_effect=lambda _kernel, _process: events.append(("job", None)) or object()), \
                    patch.object(checker, "close_windows_job"):
                checker.run("fake-cargo", [], Path(directory), {}, Path(directory) / "run.log",
                            timeout_seconds=1)
        self.assertEqual(events, [("spawn", 0x00000004), ("job", None), ("poll", None)])

    def test_check_runs_both_doctest_modes_from_the_extracted_archive(self):
        repository = Path(__file__).parents[2]
        manifest = __import__("tomllib").loads((repository / "Cargo.toml").read_text())
        package = manifest["package"]["name"]
        version = manifest["package"]["version"]

        def make_archive(cargo, root, workspace, environment, fs_path):
            archive = workspace / "target" / "package" / f"{package}-{version}.crate"
            archive.parent.mkdir(parents=True)
            content = f"[package]\nname = '{package}'\nversion = '{version}'\n".encode()
            with tarfile.open(archive, "w:gz") as output:
                member = tarfile.TarInfo(f"{package}-{version}/Cargo.toml")
                member.size = len(content)
                output.addfile(member, __import__("io").BytesIO(content))

        sibling_directory = tempfile.TemporaryDirectory()
        self.addCleanup(sibling_directory.cleanup)
        sibling_sources = make_sibling_sources(Path(sibling_directory.name))
        fs_path = sibling_sources["qubit-fs"]
        metadata_template = {
            "packages": [
                {"name": package, "source": None},
                {"name": "qubit-fs", "version": "0.3.0", "source": None,
                 "manifest_path": str(fs_path / "Cargo.toml")},
            ],
        }
        calls = []
        cargo_configs = []
        cargo_homes = []

        def record_run(cargo, arguments, cwd, environment, log, *, timeout_seconds, context):
            calls.append((arguments, cwd, environment, log, timeout_seconds, context))
            cargo_configs.append((Path(environment["CARGO_HOME"]) / "config.toml").read_text())
            cargo_homes.append(Path(environment["CARGO_HOME"]))
            if arguments[-4:] == ["metadata", "--locked", "--format-version", "1"]:
                metadata_template["packages"][0]["manifest_path"] = str(cwd / "Cargo.toml")
                return __import__("json").dumps(metadata_template)
            return ""

        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory) / "isolated"
            workspace.mkdir()
            with patch.object(checker.shutil, "which", return_value="cargo"), \
                    patch.object(checker, "sibling_fs_path", return_value=fs_path), \
                    patch.object(checker, "package_source", side_effect=make_archive), \
                    patch.object(checker, "run", side_effect=record_run):
                checker.check(repository, workspace)

        doctest_calls = [call for call in calls if "test" in call[0] and "--doc" in call[0]]
        expected_config = (
            f'[patch.crates-io]\nqubit-fs = {{ path = {__import__("json").dumps(str(fs_path))} }}\n'
        )
        self.assertEqual([call[0] for call in doctest_calls], [
            ["test", "--locked", "--doc"],
            ["test", "--locked", "--doc", "--all-features"],
        ])
        extracted = (workspace / "unpacked" / f"{package}-{version}").resolve()
        self.assertTrue(all(call[1] == extracted for call in doctest_calls))
        channel, _ = checker.load_check_settings(repository)
        expected_environment = checker.isolated_environment(workspace, channel)
        expected_environment["QUBIT_FS_SIBLING_ROOT"] = str(fs_path.parent)
        self.assertTrue(all(call[2] == expected_environment for call in doctest_calls))
        self.assertEqual([call[3].name for call in doctest_calls], [
            "doctests-default.log", "doctests-all-features.log",
        ])
        self.assertTrue(all(call[4] == 900 for call in doctest_calls))
        self.assertTrue(cargo_configs)
        self.assertTrue(all(config == expected_config for config in cargo_configs))
        self.assertEqual(len(set(cargo_homes)), 1)
        self.assertTrue(all(home.parent.name == "isolated" for home in cargo_homes))

        package_calls = []
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            package_repository = Path(sibling_directory.name) / "rs-fs-registry"
            package_repository.mkdir()
            initialize_git(package_repository)
            (package_repository / "Cargo.toml").write_text((repository / "Cargo.toml").read_text())
            subprocess.run(["git", "-C", str(package_repository), "add", "Cargo.toml"], check=True)
            subprocess.run(["git", "-C", str(package_repository), "commit", "-qm", "package inputs"], check=True)
            original_fs = fs_path
            with patch.object(checker, "run", side_effect=lambda *args, **kwargs: package_calls.append(args)):
                checker.package_source("cargo", package_repository, workspace,
                                       checker.isolated_environment(workspace, checker.load_check_settings(repository)[0]),
                                       original_fs)
            self.assertIn(__import__("json").dumps(str(original_fs)),
                          (workspace / "cargo-home/config.toml").read_text())
        self.assertEqual(package_calls[0][1], ["package", "--manifest-path",
                                                str(workspace / "package-input" / "rs-fs-registry" / "Cargo.toml"),
                                                "--allow-dirty", "--no-verify"])

        for failed_command in [
            ["test", "--locked", "--doc"],
            ["test", "--locked", "--doc", "--all-features"],
        ]:
            calls.clear()

            def fail_doctest(cargo, arguments, cwd, environment, log, *, timeout_seconds, context):
                calls.append((arguments, cwd, environment, log, timeout_seconds, context))
                if arguments[-4:] == ["metadata", "--locked", "--format-version", "1"]:
                    metadata_template["packages"][0]["manifest_path"] = str(cwd / "Cargo.toml")
                    return __import__("json").dumps(metadata_template)
                if arguments == failed_command:
                    raise RuntimeError("doctest failure marker")
                return ""

            with tempfile.TemporaryDirectory() as directory:
                failed_workspace = Path(directory) / "isolated"
                failed_workspace.mkdir()
                output = StringIO()
                with patch.object(checker.shutil, "which", return_value="cargo"), \
                        patch.object(checker, "sibling_fs_path", return_value=fs_path), \
                        patch.object(checker, "package_source", side_effect=make_archive), \
                        patch.object(checker, "run", side_effect=fail_doctest), \
                        redirect_stdout(output):
                    with self.assertRaisesRegex(RuntimeError, "doctest failure marker"):
                        checker.check(repository, failed_workspace)
                self.assertNotIn("Published archive and examples verified", output.getvalue())
            self.assertTrue(any(call[0] == failed_command for call in calls))
            test_commands = [call[0] for call in calls if "test" in call[0]]
            expected_commands = [["test", "--locked", "--doc"]]
            if failed_command[-1] == "--all-features":
                expected_commands.append(["test", "--locked", "--doc", "--all-features"])
            self.assertEqual(test_commands, expected_commands)

    @unittest.skipUnless(sys.platform != "win32", "Unix process groups are unavailable")
    def test_unix_timeout_cleanup_bounds_reaping_and_preserves_diagnostics(self):
        class FakeClock:
            now = 0.0

            def monotonic(self):
                return self.now

            def sleep(self, seconds):
                self.now += seconds

        clock = FakeClock()

        class UnreapableProcess:
            last_wait_timeouts = []

            def __init__(self, *args, stdout, **kwargs):
                self.wait_timeouts = []
                type(self).last_wait_timeouts = self.wait_timeouts
                self.pid = 1234
                self.poll_count = 0
                stdout.write("unix cleanup output marker")
                stdout.flush()

            def poll(self):
                self.poll_count += 1
                return None

            def wait(self, timeout=None):
                self.wait_timeouts.append(timeout)
                clock.now += timeout
                raise checker.subprocess.TimeoutExpired("fake process", timeout)

            def kill(self):
                raise OSError("direct kill marker")

        fake_subprocess = SimpleNamespace(Popen=UnreapableProcess,
                                          TimeoutExpired=checker.subprocess.TimeoutExpired)
        signals = []

        def fail_signal(group, received_signal):
            signals.append(received_signal)
            raise PermissionError(f"signal marker {received_signal}")

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            log = root / "unix-cleanup.log"
            with patch.object(checker, "subprocess", fake_subprocess), \
                    patch.object(checker.os, "killpg", side_effect=fail_signal), \
                    patch.object(checker.time, "sleep", side_effect=clock.sleep), \
                    patch.object(checker.time, "monotonic", side_effect=clock.monotonic):
                with self.assertRaisesRegex(RuntimeError, "signal marker|cleanup") as raised:
                    checker.run("fake-cargo", [], root, {}, log, timeout_seconds=0,
                                context="fake Unix timeout")

            self.assertEqual(signals[:2], [checker.signal.SIGTERM, checker.signal.SIGKILL], str(raised.exception))
            self.assertTrue(all(timeout is not None for timeout in UnreapableProcess.last_wait_timeouts))
            self.assertLessEqual(sum(UnreapableProcess.last_wait_timeouts), 5.9 + 1e-9)
            self.assertIn("unix cleanup output marker", log.read_text())
            self.assertRegex(str(raised.exception), "reap|signal|cleanup")

    def test_suspended_process_rollback_uses_bounded_waits(self):
        class UnreapableProcess:
            def __init__(self):
                self.wait_timeouts = []

            def poll(self):
                return None

            def kill(self):
                raise OSError("kill marker")

            def wait(self, timeout):
                self.wait_timeouts.append(timeout)
                raise checker.subprocess.TimeoutExpired("suspended child", timeout)

        process = UnreapableProcess()
        result = checker._kill_and_reap_process(process, wait_seconds=5)
        self.assertIn("wait exceeded cleanup deadline", result)
        self.assertIn("final bounded wait failed", result)
        self.assertEqual(len(process.wait_timeouts), 2)
        self.assertLessEqual(sum(process.wait_timeouts), 6)

    def test_windows_cleanup_runtime_errors_do_not_skip_timeout_log(self):
        class FakeProcess:
            def __init__(self, *args, stdout, **kwargs):
                self.returncode = None
                stdout.write("timeout output marker")
                stdout.flush()

            def poll(self):
                return self.returncode

            def kill(self):
                self.returncode = -9

            def wait(self, timeout=None):
                if self.returncode is None:
                    self.returncode = -9
                return self.returncode

        fake_subprocess = SimpleNamespace(Popen=FakeProcess, CREATE_SUSPENDED=4,
                                          TimeoutExpired=checker.subprocess.TimeoutExpired)
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "timeout.log"
            with patch.object(checker, "os", SimpleNamespace(name="nt")), \
                    patch.object(checker, "subprocess", fake_subprocess), \
                    patch.object(checker, "_windows_job_functions", return_value=object()), \
                    patch.object(checker, "_configure_windows_job", return_value=object()), \
                    patch.object(checker, "terminate_process_tree", side_effect=RuntimeError("initial close marker")), \
                    patch.object(checker, "close_windows_job", side_effect=RuntimeError("retry close marker")):
                with self.assertRaisesRegex(RuntimeError, "timed out.*retry close marker"):
                    checker.run("fake-cargo", [], Path(directory), {}, log, timeout_seconds=0,
                                context="fake Windows timeout")
            self.assertIn("timeout output marker", log.read_text())

    def test_windows_cleanup_runtime_errors_do_not_skip_normal_exit_log(self):
        class FakeProcess:
            def __init__(self, *args, stdout, **kwargs):
                self.returncode = 0
                stdout.write("normal output marker")
                stdout.flush()

            def poll(self):
                return self.returncode

        fake_subprocess = SimpleNamespace(Popen=FakeProcess, CREATE_SUSPENDED=4,
                                          TimeoutExpired=checker.subprocess.TimeoutExpired)
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "normal.log"
            with patch.object(checker, "os", SimpleNamespace(name="nt")), \
                    patch.object(checker, "subprocess", fake_subprocess), \
                    patch.object(checker, "_windows_job_functions", return_value=object()), \
                    patch.object(checker, "_configure_windows_job", return_value=object()), \
                    patch.object(checker, "close_windows_job", side_effect=RuntimeError("close marker")):
                with self.assertRaisesRegex(RuntimeError, "cleanup failed.*close marker"):
                    checker.run("fake-cargo", [], Path(directory), {}, log, timeout_seconds=2,
                                context="fake Windows normal exit")
            self.assertIn("normal output marker", log.read_text())

    def test_windows_job_close_failure_is_reported(self):
        close_results = iter([False, False, True])
        kernel32 = SimpleNamespace(CloseHandle=lambda _: next(close_results),
                                   TerminateJobObject=lambda *_: False)
        job = checker.WindowsJob(kernel32, 123)
        with patch.object(checker.ctypes, "WinError", return_value=OSError("close marker"), create=True), \
                patch.object(checker.ctypes, "get_last_error", return_value=5, create=True):
            with self.assertRaisesRegex(RuntimeError, "TerminateJobObject failed"):
                job.cleanup()
        self.assertEqual(job._handle, 123, "failed cleanup retains ownership for a later retry")
        with patch.object(checker.ctypes, "WinError", return_value=OSError("close marker"), create=True), \
                patch.object(checker.ctypes, "get_last_error", return_value=5, create=True):
            job.cleanup()
        self.assertIsNone(job._handle)

    def test_windows_job_termination_fallback_cleans_tree_after_close_failure(self):
        close_results = iter([False, True])
        kernel32 = SimpleNamespace(CloseHandle=lambda _: next(close_results),
                                   TerminateJobObject=lambda *_: True)
        job = checker.WindowsJob(kernel32, 123)
        with patch.object(checker.ctypes, "WinError", return_value=OSError("close marker"), create=True), \
                patch.object(checker.ctypes, "get_last_error", return_value=5, create=True):
            job.cleanup()
        self.assertIsNone(job._handle)

    def test_windows_thread_and_snapshot_close_failures_are_reported(self):
        kernel32 = SimpleNamespace(CloseHandle=lambda _: False)
        with patch.object(checker.ctypes, "WinError", return_value=OSError("close marker"), create=True), \
                patch.object(checker.ctypes, "get_last_error", return_value=5, create=True):
            with self.assertRaisesRegex(OSError, "primary thread"):
                checker._close_windows_handle(kernel32, 123, "primary thread")
            with self.assertRaisesRegex(OSError, "thread snapshot"):
                checker._close_windows_handle(kernel32, 123, "thread snapshot")

    def test_run_returns_success_output_and_logs_nonzero_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            success = checker.run(sys.executable, ["-c", "print('success marker')"], root, {},
                                  root / "success.log", timeout_seconds=2)
            self.assertIn("success marker", success)
            with self.assertRaisesRegex(RuntimeError, "failed with exit code 7"):
                checker.run(sys.executable, ["-c", "import sys; print('failure marker'); sys.exit(7)"],
                            root, {}, root / "failure.log", timeout_seconds=2, context="nonzero context")
            self.assertIn("failure marker", (root / "failure.log").read_text())

    def test_run_times_out_and_preserves_partial_output(self):
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "timeout.log"
            started = time.monotonic()
            with self.assertRaisesRegex(RuntimeError, "unique timeout context"):
                checker.run(
                    sys.executable,
                    ["-c", "import sys,time; print('partial marker', flush=True); time.sleep(30)"],
                    Path(directory), {}, log, timeout_seconds=0.3, context="unique timeout context",
                )
            self.assertLess(time.monotonic() - started, 5)
            self.assertIn("partial marker", log.read_text())

    def test_run_timeout_stops_grandchild(self):
        with tempfile.TemporaryDirectory() as directory:
            heartbeat = Path(directory) / "heartbeat"
            log = Path(directory) / "grandchild.log"
            child_code = "import pathlib,sys,time; p=pathlib.Path(sys.argv[1]); n=0; exec('while True: n+=1; p.write_text(str(n)); time.sleep(0.02)')"
            code = (
                "import pathlib,subprocess,sys,time; p=pathlib.Path(sys.argv[1]); p.write_text('parent'); "
                f"subprocess.Popen([sys.executable,'-c',{child_code!r},sys.argv[1]]); "
                "deadline=time.monotonic()+2; "
                "exec('while p.read_text()==\"parent\" and time.monotonic()<deadline: time.sleep(0.01)'); "
                "assert p.read_text()!=\"parent\"; print('parent marker',flush=True); time.sleep(30)"
            )
            with self.assertRaises(RuntimeError):
                checker.run(sys.executable, ["-c", code, str(heartbeat)], Path(directory), {}, log,
                            timeout_seconds=3, context="grandchild timeout")
            self.assertTrue(heartbeat.exists())
            first = heartbeat.read_text()
            self.assertNotEqual(first, "parent", "grandchild must run before timeout cleanup")
            time.sleep(0.1)
            self.assertEqual(first, heartbeat.read_text())

    def test_run_cleans_grandchild_after_parent_exits(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            heartbeat = root / "heartbeat"
            log = root / "parent-exits.log"
            child_code = (
                "import pathlib,sys,time; p=pathlib.Path(sys.argv[1]); n=0; "
                "exec('while True: n+=1; p.write_text(str(n)); time.sleep(0.02)')"
            )
            parent_code = (
                "import pathlib,subprocess,sys,time; "
                f"subprocess.Popen([sys.executable,'-c',{child_code!r},sys.argv[1]]); "
                "p=pathlib.Path(sys.argv[1]); deadline=time.monotonic()+2; "
                "exec('while not p.exists() and time.monotonic()<deadline: time.sleep(0.01)'); "
                "assert p.exists(); print('parent exited',flush=True)"
            )
            output = checker.run(sys.executable, ["-c", parent_code, str(heartbeat)], root, {}, log,
                                 timeout_seconds=3, context="parent exits before grandchild")
            self.assertIn("parent exited", output)
            self.assertTrue(heartbeat.exists())
            first = heartbeat.read_text()
            time.sleep(0.1)
            self.assertEqual(first, heartbeat.read_text())

    def test_timeout_cleanup_failure_is_reported_and_output_logged(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            log = root / "cleanup.log"
            with patch.object(checker, "terminate_process_tree", side_effect=RuntimeError("cleanup marker")):
                with self.assertRaisesRegex(RuntimeError, "cleanup marker"):
                    checker.run(sys.executable,
                                ["-c", "import sys,time; print('before cleanup',flush=True); time.sleep(30)"],
                                root, {}, log, timeout_seconds=0.2, context="cleanup context")
            self.assertIn("before cleanup", log.read_text())

    def test_main_preserves_workspace_when_check_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory) / "diagnostics"
            workspace.mkdir()
            with patch.object(checker.tempfile, "mkdtemp", return_value=str(workspace)), \
                    patch.object(checker, "check", side_effect=RuntimeError("expected failure")):
                stderr = StringIO()
                with patch("sys.argv", ["check_published_docs.py", directory]), redirect_stderr(stderr):
                    self.assertEqual(checker.main(), 1)
            self.assertTrue(workspace.is_dir())
            message = stderr.getvalue().split("Diagnostics preserved at ", 1)[1].strip()
            self.assertTrue(Path(message).samefile(workspace))

    def test_ci_runs_published_documentation_gate_for_all_required_events(self):
        repository = Path(__file__).parents[2]
        workflow_lines = (repository / ".github/workflows/ci.yml").read_text().splitlines()

        def indented_block(header: str, end_header: str | None = None) -> list[str]:
            start = workflow_lines.index(header)
            end = next(
                (index for index in range(start + 1, len(workflow_lines))
                 if (workflow_lines[index].startswith(end_header) if end_header else
                     workflow_lines[index].startswith("  ")
                     and len(workflow_lines[index]) > 2
                     and not workflow_lines[index][2].isspace())),
                len(workflow_lines),
            )
            return workflow_lines[start:end]

        triggers = indented_block("on:", "permissions:")
        self.assertIn("  pull_request:", triggers)
        self.assertIn("  push:", triggers)
        self.assertIn("      - main", triggers)
        self.assertIn("  workflow_dispatch:", triggers)
        self.assertFalse(any("verify_published_dependencies" in line for line in triggers))
        dispatch = triggers[triggers.index("  workflow_dispatch:") + 1:]
        self.assertFalse(any(line.startswith("    ") for line in dispatch))

        job = indented_block("  published-documentation:")
        self.assertFalse(any(line.startswith("    if:") for line in job))
        self.assertFalse(any(line.strip().startswith("continue-on-error:") for line in job))
        run_commands = [line.strip()[len("run:"):].strip() for line in job if line.strip().startswith("run:")]
        self.assertEqual(
            [command for command in run_commands if "check_published_docs.py" in command],
            ["python3 scripts/check_published_docs.py ."],
        )
        self.assertEqual(run_commands.count("python3 scripts/check_published_docs.py ."), 1)

        checker_path = repository / "scripts/check_published_docs.py"
        self.assertTrue(checker_path.is_file())
        result = subprocess.run(
            [sys.executable, str(checker_path), "--help"],
            cwd=repository,
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("root", result.stdout)

    def test_unused_dispatch_input_is_absent(self):
        repository = Path(__file__).parents[2]
        workflow = (repository / ".github/workflows/ci.yml").read_text()
        self.assertNotIn("verify_published_dependencies", workflow)

    def test_package_snapshot_uses_git_inputs_and_supported_path_layout(self):
        with tempfile.TemporaryDirectory() as directory:
            temporary = Path(directory)
            source = temporary / "project"
            source.mkdir()
            initialize_git(source)
            (source / "Cargo.toml").write_text('[package]\nname="fixture"\nversion="0.1.0"\n[dependencies]\nqubit-fs={path="../rs-fs"}\n')
            (source / "src").mkdir()
            (source / "src/lib.rs").write_text("// embedded source\n")
            (source / "tests/support").mkdir(parents=True)
            (source / "tests/support/data.txt").write_text("support\n")
            (source / "ignored.txt").write_text("ignored\n")
            (source / "target").mkdir()
            (source / "target/build.txt").write_text("build\n")
            (source / ".cargo").mkdir()
            (source / ".cargo/config.toml").write_text("[build]\ntarget-dir='unsafe'\n")
            (source / "gone.txt").write_text("tracked then removed\n")
            subprocess.run(["git", "-C", str(source), "add", "Cargo.toml", "src", "tests", "gone.txt"], check=True)
            subprocess.run(["git", "-C", str(source), "commit", "-qm", "inputs"], check=True)
            (source / "gone.txt").unlink()
            (source / "untracked.txt").write_text("include me\n")
            fs_path = temporary / "rs-fs"
            fs_path.mkdir()
            initialize_git(fs_path)
            (fs_path / "Cargo.toml").write_text('[package]\nname="qubit-fs"\nversion="0.3.0"\n')
            subprocess.run(["git", "-C", str(fs_path), "add", "Cargo.toml"], check=True)
            subprocess.run(["git", "-C", str(fs_path), "commit", "-qm", "inputs"], check=True)
            workspace = temporary / "isolated"
            workspace.mkdir()
            snapshot = checker.prepare_package_source(source, workspace, fs_path)
            self.assertEqual(snapshot, workspace / "package-input" / "rs-fs-registry")
            self.assertEqual((snapshot / "src/lib.rs").read_text(), "// embedded source\n")
            self.assertEqual((snapshot / "tests/support/data.txt").read_text(), "support\n")
            self.assertEqual((snapshot / "untracked.txt").read_text(), "include me\n")
            self.assertFalse((snapshot / "gone.txt").exists())
            self.assertFalse((snapshot / "ignored.txt").exists())
            self.assertFalse((snapshot / "target").exists())
            self.assertFalse((snapshot / ".cargo").exists())
            self.assertEqual((snapshot.parent / "rs-fs/Cargo.toml").read_text(), (fs_path / "Cargo.toml").read_text())
            self.assertEqual((snapshot / "Cargo.toml").read_text().count('path="../rs-fs"'), 1)
            (source / "Cargo.toml").write_text(
                '[package]\nname="fixture"\nversion="0.1.0"\n[dependencies]\nother={path="../external"}\n'
            )
            with self.assertRaisesRegex(ValueError, r"unsupported path dependency other: ../external"):
                checker.prepare_package_source(source, workspace, fs_path)

    def test_package_snapshot_rejects_symbolic_links_with_path(self):
        if sys.platform == "win32":
            self.skipTest("symlink creation may require elevated privileges on Windows")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "project"
            root.mkdir()
            initialize_git(root)
            target = Path(directory) / "outside.txt"
            target.write_text("outside")
            (root / "link.txt").symlink_to(target)
            with self.assertRaisesRegex(ValueError, "link.txt"):
                checker.prepare_package_source(root, Path(directory) / "workspace", Path(directory) / "rs-fs")

    def test_packaging_does_not_overwrite_concurrent_source_lock_on_success_or_failure(self):
        for outcome in ("success", "failure", "timeout"):
            with self.subTest(outcome=outcome), tempfile.TemporaryDirectory() as directory:
                root = Path(directory) / "project"
                root.mkdir()
                initialize_git(root)
                (root / "Cargo.toml").write_text('[package]\nname="fixture"\nversion="0.1.0"\n')
                (root / "Cargo.lock").write_text("initial source lock\n")
                workspace = Path(directory) / "isolated"
                workspace.mkdir()
                fs_path = Path(directory) / "rs-fs"
                fs_path.mkdir()
                initialize_git(fs_path)
                (fs_path / "Cargo.toml").write_text('[package]\nname="qubit-fs"\nversion="0.3.0"\n')
                subprocess.run(["git", "-C", str(fs_path), "add", "Cargo.toml"], check=True)
                subprocess.run(["git", "-C", str(fs_path), "commit", "-qm", "inputs"], check=True)
                environment = {"CARGO_HOME": str(workspace / "cargo-home")}

                def fake_cargo(_cargo, arguments, *_args, **kwargs):
                    manifest = Path(arguments[arguments.index("--manifest-path") + 1])
                    self.assertNotEqual(manifest.parent.resolve(), root.resolve())
                    (manifest.parent / "Cargo.lock").write_text("generated snapshot lock\n")
                    (root / "Cargo.lock").write_text("concurrent source update\n")
                    if outcome == "failure":
                        raise RuntimeError("package failed")
                    if outcome == "timeout":
                        raise RuntimeError("Cargo package timed out")

                expected_error = {"success": None, "failure": "package failed", "timeout": "timed out"}[outcome]
                if expected_error:
                    with patch.object(checker, "run", side_effect=fake_cargo), self.assertRaisesRegex(RuntimeError, expected_error):
                        checker.package_source("cargo", root, workspace, environment, fs_path)
                else:
                    with patch.object(checker, "run", side_effect=fake_cargo):
                        checker.package_source("cargo", root, workspace, environment, fs_path)
                self.assertEqual((root / "Cargo.lock").read_text(), "concurrent source update\n")

    def test_packaging_keeps_absent_source_lock_absent(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "project"
            root.mkdir()
            initialize_git(root)
            (root / "Cargo.toml").write_text('[package]\nname="fixture"\nversion="0.1.0"\n')
            fs_path = Path(directory) / "rs-fs"
            fs_path.mkdir()
            initialize_git(fs_path)
            (fs_path / "Cargo.toml").write_text('[package]\nname="qubit-fs"\nversion="0.3.0"\n')
            subprocess.run(["git", "-C", str(fs_path), "add", "Cargo.toml"], check=True)
            subprocess.run(["git", "-C", str(fs_path), "commit", "-qm", "inputs"], check=True)
            workspace = Path(directory) / "isolated"
            workspace.mkdir()
            with patch.object(checker, "run", return_value=""):
                checker.package_source("cargo", root, workspace,
                                       {"CARGO_HOME": str(workspace / "cargo-home")}, fs_path)
            self.assertFalse((root / "Cargo.lock").exists())

    def test_environment_drops_inherited_build_overrides(self):
        with patch.dict("os.environ", {"CARGO_HOME": "/old", "CARGO_ENCODED_RUSTFLAGS": "unsafe", "RUSTFLAGS": "unsafe", "CARGO_REGISTRIES_CRATES_IO_INDEX": "local", "RUSTC_WRAPPER": "wrapper"}):
            env = checker.isolated_environment(Path("/isolated"), "1.94.0")
        self.assertEqual(Path(env["CARGO_HOME"]), Path("/isolated") / "cargo-home")
        self.assertEqual(env["QUBIT_FS_REGISTRY_DOC_DEPS"], "published")
        for key in ["CARGO_ENCODED_RUSTFLAGS", "CARGO_REGISTRIES_CRATES_IO_INDEX", "RUSTFLAGS", "RUSTC_WRAPPER"]:
            self.assertNotIn(key, env)

    def test_manifest_rejects_all_active_source_overrides(self):
        for source in ["path", "git", "workspace"]:
            for section in ["dependencies", "dev-dependencies", "build-dependencies"]:
                with self.assertRaises(ValueError):
                    checker.validate_manifest({section: {"qubit-fs": {source: "bad", "version": "0.3"}}})
        with self.assertRaises(ValueError):
            checker.validate_manifest({"target": {"cfg(unix)": {"dependencies": {"x": {"path": "../x"}}}}})
        for key in ["patch", "replace", "workspace"]:
            with self.assertRaises(ValueError):
                checker.validate_manifest({key: {}})
        checker.validate_manifest({"package": {"metadata": {"documentation": {"path": "../inert"}}}, "dependencies": {"qubit-fs": {"version": "0.3"}}})

    def test_graph_rejects_duplicate_versions_and_wrong_sources(self):
        temporary_root = tempfile.TemporaryDirectory()
        self.addCleanup(temporary_root.cleanup)
        root = Path(temporary_root.name) / "package"
        root.mkdir()
        sibling_directory = tempfile.TemporaryDirectory()
        self.addCleanup(sibling_directory.cleanup)
        sibling_sources = make_sibling_sources(Path(sibling_directory.name))
        fs_path = sibling_sources["qubit-fs"]
        fs_local_path = sibling_sources["qubit-fs-local"]
        local_files_path = sibling_sources["qubit-local-files"]
        allowed_paths = {
            "qubit-fs": fs_path,
            "qubit-fs-local": fs_local_path,
            "qubit-local-files": local_files_path,
        }
        own = {"name": "qubit-fs-registry", "source": None, "manifest_path": str(root / "Cargo.toml")}
        fs = {"name": "qubit-fs", "version": "0.3.0", "source": None,
              "manifest_path": str(fs_path / "Cargo.toml")}
        fs_local = {"name": "qubit-fs-local", "version": "0.11.0", "source": None,
                    "manifest_path": str(fs_local_path / "Cargo.toml")}
        local_files = {"name": "qubit-local-files", "version": "0.5.0", "source": None,
                       "manifest_path": str(local_files_path / "Cargo.toml")}
        checker.validate_graph({"packages": [own, fs, fs_local, local_files]}, "qubit-fs-registry", root, allowed_paths)
        for bad in [
            dict(fs, manifest_path="/unlisted/rs-fs/Cargo.toml"),
            dict(fs_local, source="git+https://example.invalid/repo"),
        ]:
            with self.assertRaises(ValueError):
                checker.validate_graph({"packages": [own, bad]}, "qubit-fs-registry", root, allowed_paths)
        with self.assertRaises(ValueError):
            checker.validate_graph({"packages": [own, fs, dict(fs, version="0.2")]}, "qubit-fs-registry", root, allowed_paths)
        for unexpected_name in ["qubit-unlisted", "ordinary-local"]:
            unexpected = {
                "name": unexpected_name,
                "version": "1.0.0",
                "source": None,
                "manifest_path": "/unlisted/Cargo.toml",
            }
            with self.assertRaisesRegex(ValueError, "unexpected local dependency"):
                checker.validate_graph({"packages": [own, unexpected]}, "qubit-fs-registry", root,
                                       allowed_paths)
        with self.assertRaisesRegex(ValueError, "unrecognized local dependency allowlist"):
            checker.validate_graph({"packages": [own]}, "qubit-fs-registry", root,
                                   {"qubit-unlisted": fs_path})


if __name__ == "__main__":
    unittest.main()
